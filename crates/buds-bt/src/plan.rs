//! Which RFCOMM service / channel to try, in which order, and when to give up. Pure logic shared by
//! the OS backends; only the actual connect call is platform specific.

use std::fmt;

/// One way of reaching the buds' control channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate {
    /// HeyMelody control service `0000079A-D102-11E1-9B23-00025B00A5A5` (the OS does the SDP lookup).
    HeyMelody,
    /// HeyMelody control service `00001107-D102-11E1-9B23-00025B00A5A5` (other generation).
    HeyMelodyAlt,
    /// Standard serial port `00001101-...`: realme buds, when no HeyMelody service exists.
    Spp,
    /// A fixed RFCOMM channel, guessed when no service record is found (15 on the Buds 4).
    Channel(u8),
}

impl Candidate {
    /// Channels guessed after every service lookup came back "not found".
    pub const FALLBACK_CHANNELS: [u8; 3] = [15, 12, 13];

    /// Short text stored in settings as the connection hint, and printed in logs.
    pub fn name(self) -> String {
        match self {
            Candidate::HeyMelody => "heymelody".into(),
            Candidate::HeyMelodyAlt => "heymelody-alt".into(),
            Candidate::Spp => "spp".into(),
            Candidate::Channel(n) => format!("channel:{n}"),
        }
    }

    pub fn from_name(text: &str) -> Option<Candidate> {
        match text {
            "heymelody" => Some(Candidate::HeyMelody),
            "heymelody-alt" => Some(Candidate::HeyMelodyAlt),
            "spp" => Some(Candidate::Spp),
            other => other
                .strip_prefix("channel:")
                .and_then(|n| n.parse().ok())
                .map(Candidate::Channel),
        }
    }

    /// How long to wait for this candidate before moving on.
    pub fn timeout(self) -> std::time::Duration {
        match self {
            Candidate::Channel(_) => std::time::Duration::from_secs(3),
            _ => std::time::Duration::from_secs(5),
        }
    }
}

impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Candidate::HeyMelody => f.write_str("HeyMelody service 079A"),
            Candidate::HeyMelodyAlt => f.write_str("HeyMelody service 1107"),
            Candidate::Spp => f.write_str("serial port service"),
            Candidate::Channel(n) => write!(f, "RFCOMM channel {n}"),
        }
    }
}

/// Why one connect attempt failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectError {
    /// OS error code (Winsock error number on Windows), 0 if none.
    pub code: i32,
    pub message: String,
    pub timed_out: bool,
    /// The caller gave up (the controller dropped the open); stop at once.
    pub cancelled: bool,
}

impl ConnectError {
    pub fn timeout() -> ConnectError {
        ConnectError {
            code: 0,
            message: "timed out".into(),
            timed_out: true,
            cancelled: false,
        }
    }

    pub fn cancelled() -> ConnectError {
        ConnectError {
            code: 0,
            message: "cancelled".into(),
            timed_out: false,
            cancelled: true,
        }
    }

    /// The service isn't there (as opposed to the device being unreachable): Winsock 10108
    /// `WSASERVICE_NOT_FOUND`, and the address / argument errors some stacks return for an unknown
    /// service class (10049, 10022). Worth trying the next candidate.
    pub fn is_not_found(&self) -> bool {
        matches!(self.code, 10108 | 10049 | 10022)
    }

    /// The service exists but its one connection is taken (Winsock 10048 `WSAEADDRINUSE`): another
    /// program, or a connection of ours that the buds haven't released yet. Trying the other
    /// services would only find unrelated ones, so the walk ends and the controller retries later.
    pub fn is_busy(&self) -> bool {
        self.code == 10048
    }
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code != 0 {
            write!(f, "{} (error {})", self.message, self.code)
        } else {
            f.write_str(&self.message)
        }
    }
}

/// Tries candidates in order until `connect` succeeds.
///
/// 1. The candidate that worked last time (`hint`), if any; a failure there never ends the walk.
/// 2. HeyMelody 079A, then 1107. Anything but "service not found" (timeout, host down) ends the walk
///    with an error so the controller can retry later instead of burning time on more guesses.
/// 3. Only if both said "not found": the standard serial port (realme).
/// 4. Only if that said "not found" too: channels 15, 12, 13; every failure moves to the next.
pub fn walk<T>(
    hint: Option<&str>,
    mut connect: impl FnMut(Candidate) -> Result<T, ConnectError>,
    mut log: impl FnMut(&str),
) -> Result<(Candidate, T), ConnectError> {
    let mut tried: Vec<Candidate> = Vec::new();
    let mut attempt =
        |candidate: Candidate, log: &mut dyn FnMut(&str)| -> Option<Result<T, ConnectError>> {
            if tried.contains(&candidate) {
                return None;
            }
            tried.push(candidate);
            log(&format!("trying {candidate}"));
            let result = connect(candidate);
            if let Err(error) = &result {
                log(&format!("{candidate}: {error}"));
            }
            Some(result)
        };

    if let Some(candidate) = hint.and_then(Candidate::from_name) {
        match attempt(candidate, &mut log) {
            Some(Ok(value)) => return Ok((candidate, value)),
            Some(Err(e)) if e.cancelled || e.is_busy() => return Err(e),
            _ => {}
        }
    }

    let mut last = ConnectError {
        code: 0,
        message: "no candidate to try".into(),
        timed_out: false,
        cancelled: false,
    };
    for candidate in [
        Candidate::HeyMelody,
        Candidate::HeyMelodyAlt,
        Candidate::Spp,
    ] {
        match attempt(candidate, &mut log) {
            None => {}
            Some(Ok(value)) => return Ok((candidate, value)),
            Some(Err(e)) if e.cancelled || !e.is_not_found() => return Err(e),
            Some(Err(e)) => last = e,
        }
    }
    for channel in Candidate::FALLBACK_CHANNELS {
        match attempt(Candidate::Channel(channel), &mut log) {
            None => {}
            Some(Ok(value)) => return Ok((Candidate::Channel(channel), value)),
            Some(Err(e)) if e.cancelled => return Err(e),
            Some(Err(e)) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn not_found() -> ConnectError {
        ConnectError {
            code: 10108,
            message: "service not found".into(),
            timed_out: false,
            cancelled: false,
        }
    }

    /// Runs `walk` with a scripted connect function; returns the candidates tried and the result.
    fn run(
        hint: Option<&str>,
        script: impl Fn(Candidate) -> Result<(), ConnectError>,
    ) -> (Vec<Candidate>, Result<Candidate, ConnectError>) {
        let mut tried = Vec::new();
        let result = walk(
            hint,
            |c| {
                tried.push(c);
                script(c)
            },
            |_| {},
        );
        (tried, result.map(|(c, ())| c))
    }

    #[test]
    fn first_service_that_works_wins() {
        let (tried, result) = run(None, |_| Ok(()));
        assert_eq!(tried, [Candidate::HeyMelody]);
        assert_eq!(result.unwrap(), Candidate::HeyMelody);
    }

    #[test]
    fn second_service_is_tried_when_the_first_is_not_found() {
        let (tried, result) = run(None, |c| {
            if c == Candidate::HeyMelodyAlt {
                Ok(())
            } else {
                Err(not_found())
            }
        });
        assert_eq!(tried, [Candidate::HeyMelody, Candidate::HeyMelodyAlt]);
        assert_eq!(result.unwrap(), Candidate::HeyMelodyAlt);
    }

    #[test]
    fn everything_not_found_walks_spp_then_channels_15_12_13() {
        let (tried, result) = run(None, |_| Err(not_found()));
        assert_eq!(
            tried,
            [
                Candidate::HeyMelody,
                Candidate::HeyMelodyAlt,
                Candidate::Spp,
                Candidate::Channel(15),
                Candidate::Channel(12),
                Candidate::Channel(13),
            ]
        );
        assert!(result.is_err());
    }

    #[test]
    fn channel_15_is_found_after_the_services_fail() {
        let (tried, result) = run(None, |c| {
            if c == Candidate::Channel(15) {
                Ok(())
            } else {
                Err(not_found())
            }
        });
        assert_eq!(result.unwrap(), Candidate::Channel(15));
        assert_eq!(tried.len(), 4);
    }

    #[test]
    fn a_timeout_on_a_service_ends_the_walk() {
        let (tried, result) = run(None, |_| Err(ConnectError::timeout()));
        assert_eq!(tried, [Candidate::HeyMelody]);
        assert!(result.unwrap_err().timed_out);
    }

    #[test]
    fn host_down_after_not_found_ends_the_walk_before_the_channels() {
        let down = ConnectError {
            code: 10064,
            message: "host is down".into(),
            timed_out: false,
            cancelled: false,
        };
        let (tried, result) = run(None, |c| {
            if c == Candidate::HeyMelody {
                Err(not_found())
            } else {
                Err(down.clone())
            }
        });
        assert_eq!(tried, [Candidate::HeyMelody, Candidate::HeyMelodyAlt]);
        assert_eq!(result.unwrap_err().code, 10064);
    }

    #[test]
    fn the_hint_goes_first_and_is_not_repeated() {
        let (tried, result) = run(Some("channel:15"), |_| Ok(()));
        assert_eq!(tried, [Candidate::Channel(15)]);
        assert_eq!(result.unwrap(), Candidate::Channel(15));

        // A failing hint never ends the walk; the normal order follows, skipping the hint.
        let (tried, _) = run(Some("heymelody"), |_| Err(not_found()));
        assert_eq!(tried[0], Candidate::HeyMelody);
        assert_eq!(
            tried.iter().filter(|c| **c == Candidate::HeyMelody).count(),
            1
        );
        assert_eq!(tried.len(), 6);
        let (tried, _) = run(Some("channel:15"), |c| {
            if c == Candidate::Channel(15) {
                Err(ConnectError::timeout())
            } else {
                Err(not_found())
            }
        });
        assert_eq!(tried[0], Candidate::Channel(15));
        assert_eq!(
            tried
                .iter()
                .filter(|c| **c == Candidate::Channel(15))
                .count(),
            1
        );
    }

    #[test]
    fn a_busy_control_service_ends_the_walk_instead_of_finding_unrelated_services() {
        let busy = ConnectError {
            code: 10048,
            message: "address in use".into(),
            timed_out: false,
            cancelled: false,
        };
        // Without a hint: 079A busy -> stop (never reaches the serial port service).
        let (tried, result) = run(None, |_| Err(busy.clone()));
        assert_eq!(tried, [Candidate::HeyMelody]);
        assert_eq!(result.unwrap_err().code, 10048);
        // With a hint: the same.
        let (tried, result) = run(Some("heymelody"), |_| Err(busy.clone()));
        assert_eq!(tried, [Candidate::HeyMelody]);
        assert!(result.is_err());
    }

    #[test]
    fn cancellation_stops_immediately() {
        let (tried, result) = run(None, |_| Err(ConnectError::cancelled()));
        assert_eq!(tried.len(), 1);
        assert!(result.unwrap_err().cancelled);
    }

    #[test]
    fn candidate_names_round_trip() {
        for c in [
            Candidate::HeyMelody,
            Candidate::HeyMelodyAlt,
            Candidate::Spp,
            Candidate::Channel(15),
        ] {
            assert_eq!(Candidate::from_name(&c.name()), Some(c));
        }
        assert_eq!(Candidate::from_name("nonsense"), None);
        assert_eq!(Candidate::from_name("channel:x"), None);
    }
}
