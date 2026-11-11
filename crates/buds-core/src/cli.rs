//! The command-line commands, shared by `budsctl` (own controller) and the app's IPC server (live
//! controller). Output format follows the original app's `--cli`, `--watch`, `--scan` and `--raw`
//! modes.
//!
//! ```text
//! status | anc | transparency | off | smart | max | moderate | mild | adaptive | cycle
//! eq=<id> | custom=<g1,g2,..> | lowlatency=on|off | find=on|off | gesture=<side>,<gesture>,<action>
//! watch [seconds] | scan [label] | raw CMD:PAYLOAD [CMD:PAYLOAD ...] | sdp
//! ```
//!
//! The upstream spellings work too: `--cli anc`, `--watch 30`, `--scan x`, `--raw 0106`.

use std::collections::BTreeMap;
use std::io::Write;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tokio::time::{Instant, sleep, timeout};

use crate::controller::{ControllerHandle, Event, Snapshot};
use crate::packet_log::LogEntry;
use crate::platform::LogDirection;
use crate::protocol::{parse_hex, to_hex};
use crate::types::*;

/// How log lines are prefixed while a command runs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Echo {
    /// Not echoed.
    Off,
    /// `[  123ms] -> AA ..`
    Millis,
    /// `[  1.23s] -> AA ..`
    Seconds,
}

enum Command {
    /// A mode / level / `key=value` / `status`.
    Control(String),
    Watch(f64),
    Scan(String),
    Raw(Vec<(u16, Vec<u8>)>),
    Sdp,
}

/// Runs one command line against a controller, writing its output to `out`. Returns the process
/// exit code (0 = success).
pub async fn execute(
    handle: &ControllerHandle,
    args: &[String],
    out: &mut (dyn Write + Send),
) -> i32 {
    let command = match parse_args(args) {
        Ok(c) => c,
        Err(message) => {
            let _ = writeln!(out, "{message}");
            return 2;
        }
    };
    let echo = match &command {
        Command::Control(_) => Echo::Millis,
        Command::Watch(_) | Command::Raw(_) => Echo::Seconds,
        Command::Scan(_) | Command::Sdp => Echo::Off,
    };

    let started = Instant::now();
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    let mut log_rx = handle.log().subscribe();
    let mut script = Box::pin(run(handle, command, line_tx));
    let exit = loop {
        tokio::select! {
            code = &mut script => break code,
            Some(line) = line_rx.recv() => { let _ = writeln!(out, "{line}"); }
            entry = log_rx.recv() => {
                if let Ok(entry) = entry { echo_entry(out, echo, started, &entry); }
            }
        }
    };
    while let Ok(entry) = log_rx.try_recv() {
        echo_entry(out, echo, started, &entry);
    }
    while let Ok(line) = line_rx.try_recv() {
        let _ = writeln!(out, "{line}");
    }
    exit
}

fn echo_entry(out: &mut (dyn Write + Send), echo: Echo, started: Instant, entry: &LogEntry) {
    let elapsed = started.elapsed().as_secs_f64();
    let stamp = match echo {
        Echo::Off => return,
        Echo::Millis => format!("[{:5.0}ms] ", elapsed * 1000.0),
        Echo::Seconds => format!("[{elapsed:6.2}s] "),
    };
    let arrow = match entry.direction {
        LogDirection::Sent => "-> ",
        LogDirection::Received => "<- ",
        LogDirection::Info => "",
    };
    let _ = writeln!(out, "{stamp}{arrow}{}", entry.text);
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut args: Vec<&str> = args.iter().map(String::as_str).collect();
    if args.first() == Some(&"--cli") {
        args.remove(0);
    }
    let head = args.first().copied().unwrap_or("status");
    let rest = &args[args.len().min(1)..];
    match head.trim_start_matches("--") {
        "watch" => Ok(Command::Watch(
            rest.first().and_then(|s| s.parse().ok()).unwrap_or(60.0),
        )),
        "scan" => Ok(Command::Scan(
            rest.first().copied().unwrap_or("scan").to_string(),
        )),
        "sdp" => Ok(Command::Sdp),
        "raw" => {
            if rest.is_empty() {
                return Err(
                    "usage: raw CMD:PAYLOAD [CMD:PAYLOAD ...]  (hex, e.g. 0106 or 010C:0101)"
                        .into(),
                );
            }
            let mut commands = Vec::new();
            for item in rest {
                let (command, payload) = item.split_once(':').unwrap_or((item, ""));
                let command = u16::from_str_radix(command, 16)
                    .map_err(|_| format!("bad command number: {command}"))?;
                commands.push((command, parse_hex(payload)));
            }
            Ok(Command::Raw(commands))
        }
        _ => Ok(Command::Control(head.to_string())),
    }
}

type Printer = mpsc::UnboundedSender<String>;

async fn run(handle: &ControllerHandle, command: Command, print: Printer) -> i32 {
    match command {
        Command::Control(action) => control(handle, &action, &print).await,
        Command::Watch(seconds) => watch(handle, seconds, &print).await,
        Command::Scan(label) => scan(handle, &label, &print).await,
        Command::Raw(commands) => raw(handle, commands).await,
        Command::Sdp => {
            let _ =
                print.send("sdp: service discovery is not available on this platform yet".into());
            1
        }
    }
}

/// Waits until `ready` holds for the published state, or the timeout passes. Returns whether it held.
async fn wait_for(
    handle: &ControllerHandle,
    limit: Duration,
    ready: impl Fn(&Snapshot) -> bool,
) -> bool {
    let mut rx = handle.watch();
    timeout(limit, async {
        loop {
            if ready(&rx.borrow_and_update()) {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    })
    .await
    .is_ok()
}

/// Connected and the first readings are in (the rest of the start-up replies follow within ms).
fn is_ready(s: &Snapshot) -> bool {
    s.link == LinkState::Connected && s.product_id.is_some() && s.mode.is_some() && s.left.is_some()
}

async fn control(handle: &ControllerHandle, action: &str, print: &Printer) -> i32 {
    // Like a shortcut press: request the mode right away, before the link is even up.
    if let Some(mode) = NoiseMode::from_key(action) {
        handle.set_mode(mode, None);
    } else if let Some(level) = AncLevel::from_key(action) {
        handle.set_mode(NoiseMode::Anc, Some(level));
    }
    // Attached to a controller that has been running for a while (the tray app), everything is
    // already known: no need to wait for the start-up replies.
    let running = is_ready(&handle.snapshot());
    handle.start();

    let ready = wait_for(handle, Duration::from_secs(8), is_ready).await;
    if ready && !running {
        // Remaining start-up replies (EQ, gestures, switches) arrive right behind the first ones.
        sleep(Duration::from_millis(800)).await;
    }

    if action == "cycle" && ready {
        handle.cycle_mode();
    } else if let Some((key, value)) = action.split_once('=') {
        match key {
            "eq" => match value.parse::<u8>() {
                Ok(id) => handle.set_eq(id),
                Err(_) => {
                    let _ = print.send(format!("bad eq id {value}"));
                }
            },
            "custom" => handle.set_custom_eq(
                value
                    .split(',')
                    .filter_map(|g| g.trim().parse().ok())
                    .collect(),
            ),
            "lowlatency" => handle.set_low_latency(value == "on"),
            "find" => handle.find_buds(value == "on"),
            "gesture" => {
                let parts: Vec<&str> = value.split(',').collect();
                match (
                    parts.first().and_then(|s| EarSide::from_key(s)),
                    parts.get(1).and_then(|s| Gesture::from_key(s)),
                    parts.get(2).and_then(|s| GestureAction::from_key(s)),
                ) {
                    (Some(side), Some(gesture), Some(act)) if parts.len() == 3 => {
                        handle.set_gesture(side, gesture, act);
                    }
                    _ => {
                        let _ =
                            print.send(format!("bad gesture {value} (use side,gesture,action)"));
                    }
                }
            }
            other => {
                let _ = print.send(format!("unknown action {other}"));
            }
        }
    }
    // Give the buds a moment to apply and confirm before reading the state back (a plain `status`
    // changed nothing, so there is nothing to wait for).
    let changed = action != "status";
    sleep(if ready && changed {
        Duration::from_millis(1200)
    } else {
        Duration::ZERO
    })
    .await;

    let s = handle.snapshot();
    for line in status_block(&s) {
        let _ = print.send(line);
    }
    if s.link == LinkState::Connected { 0 } else { 1 }
}

/// The `status` output, line by line.
pub fn status_block(s: &Snapshot) -> Vec<String> {
    let mut lines = Vec::new();
    let link = match s.link {
        LinkState::Connected => "connected",
        LinkState::Connecting => "connecting",
        LinkState::Searching => "searching",
    };
    lines.push(format!(
        "device:  {} [{}] {link}",
        s.device_name,
        s.product_id.as_deref().unwrap_or("?")
    ));
    lines.push(format!(
        "mode:    {}{}{}",
        s.mode.map_or("unknown", NoiseMode::key),
        s.level.map_or(String::new(), |l| format!(" ({})", l.key())),
        s.noise_bit.map_or(String::new(), |b| format!(" [bit {b}]"))
    ));
    let pct = |b: Option<Battery>| b.map_or(-1, |b| i32::from(b.level));
    lines.push(format!(
        "battery: L {}%  R {}%  case {}%",
        pct(s.left),
        pct(s.right),
        pct(s.case)
    ));
    let eq_name = s
        .eq_choices
        .iter()
        .find(|c| Some(c.id) == s.eq_id)
        .map_or("", |c| c.name.as_str());
    let bands = s.active_custom_eq.as_ref().map_or(String::new(), |eq| {
        let list: Vec<String> = eq
            .bands
            .iter()
            .map(|b| format!("{}Hz:{}", b.hz, b.gain))
            .collect();
        format!(" {}", list.join(" "))
    });
    lines.push(format!(
        "eq:      {} {eq_name}{bands}",
        s.eq_id.map_or("?".to_string(), |id| id.to_string())
    ));
    lines.push(format!(
        "lowlat:  {}",
        s.low_latency
            .map_or("?", |on| if on { "on" } else { "off" })
    ));
    lines.push(format!(
        "model:   {}, capability bits: {}",
        if s.model_name.is_empty() {
            "not in catalog"
        } else {
            s.model_name.as_str()
        },
        s.capabilities_count
            .map_or("none".to_string(), |n| n.to_string())
    ));
    let quoted = |items: Vec<&str>| {
        format!(
            "[{}]",
            items
                .iter()
                .map(|i| format!("\"{i}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    lines.push(format!(
        "modes:   {} levels: {}",
        quoted(s.available_modes.iter().map(|m| m.key()).collect()),
        quoted(s.available_levels.iter().map(|l| l.key()).collect())
    ));
    let shows: Vec<&str> = DeviceFeature::ALL
        .into_iter()
        .filter(|f| s.supports.get(*f))
        .map(DeviceFeature::key)
        .collect();
    lines.push(format!("shows:   {}", shows.join(" ")));
    for side in EarSide::ALL {
        let parts: Vec<String> = Gesture::ALL
            .into_iter()
            .map(|g| {
                let action = s
                    .gestures
                    .side(side)
                    .and_then(|sg| sg.get(g))
                    .map_or("?", |slot| slot.current.key());
                format!("{}={action}", g.key())
            })
            .collect();
        lines.push(format!("gesture: {} {}", side.key(), parts.join(" ")));
    }
    lines
}

async fn watch(handle: &ControllerHandle, seconds: f64, print: &Printer) -> i32 {
    let started = Instant::now();
    let mut events = handle.events();
    handle.start();
    let deadline = sleep(Duration::from_secs_f64(seconds.max(0.0)));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            () = &mut deadline => break,
            event = events.recv() => match event {
                Ok(Event::Hosts(hosts)) => {
                    let list: Vec<String> = hosts
                        .iter()
                        .map(|h| format!("{} connected={} self={}", h.name, h.connected, h.is_this_host))
                        .collect();
                    let _ = print.send(format!("[{:6.2}s] HOSTS {}", started.elapsed().as_secs_f64(), list.join(" | ")));
                }
                Ok(Event::Frame(_)) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    0
}

async fn scan(handle: &ControllerHandle, label: &str, print: &Printer) -> i32 {
    let mut events = handle.events();
    let mut replies: BTreeMap<u16, Vec<u8>> = BTreeMap::new();
    handle.start();
    // Collect everything the buds send for the whole run.
    let mut queries: Vec<(u16, Vec<u8>)> = (0x0101u16..=0x0120).map(|c| (c, vec![])).collect();
    queries.push((0x010C, vec![0x01, 0x01]));
    queries.push((0x010D, std::iter::once(0x2F).chain(1..=0x2F).collect()));
    let start_at = sleep(Duration::from_secs(4));
    tokio::pin!(start_at);
    let mut sent = false;
    let total = Duration::from_secs(4)
        + Duration::from_millis(90) * queries.len() as u32
        + Duration::from_secs(2);
    let end = sleep(total);
    tokio::pin!(end);
    loop {
        tokio::select! {
            () = &mut end => break,
            () = &mut start_at, if !sent => {
                sent = true;
                for (n, (command, payload)) in queries.iter().cloned().enumerate() {
                    let handle = handle.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_millis(90) * n as u32).await;
                        handle.send_raw(command, payload);
                    });
                }
            }
            event = events.recv() => match event {
                Ok(Event::Frame(frame)) => { replies.insert(frame.command, frame.payload); }
                Ok(Event::Hosts(_)) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    for (command, payload) in &replies {
        let _ = print.send(format!("{label} 0x{command:04X} {}", to_hex(payload)));
    }
    if handle.snapshot().link == LinkState::Connected {
        0
    } else {
        1
    }
}

async fn raw(handle: &ControllerHandle, commands: Vec<(u16, Vec<u8>)>) -> i32 {
    handle.start();
    // Wait for the link, then send each command 2.5 s apart so replies are easy to attribute.
    wait_for(handle, Duration::from_secs(8), |s| {
        s.link == LinkState::Connected
    })
    .await;
    sleep(Duration::from_secs(1)).await;
    for (n, (command, payload)) in commands.into_iter().enumerate() {
        if n > 0 {
            sleep(Duration::from_millis(2500)).await;
        }
        handle.send_raw(command, payload);
    }
    sleep(Duration::from_secs(2)).await;
    0
}
