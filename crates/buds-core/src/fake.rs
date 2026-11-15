//! A simulated Buds 4 for building the UI without hardware, for `budsctl --fake`, and for the
//! controller tests. Enable with the `fake` cargo feature.
//!
//! `FakePlatform` answers the HeyMelody commands like the real buds (replies immediately; pushes
//! about 0.5 s after a change) and has knobs to make things go wrong: dropped replies, a hanging
//! open, a closed link, the buds only connected to the phone, a clock jump (sleep), ...

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tokio::time::Instant;

use crate::platform::{BtDevice, Link, LinkSink, LogDirection, LogFn, OpenError, Platform};
use crate::protocol::{Frame, FrameParser, cmd, push, reply};
use crate::types::*;

/// How long after a change the fake buds send the confirming push.
const PUSH_DELAY: Duration = Duration::from_millis(500);

/// Address of the simulated buds.
pub const FAKE_ADDRESS: [u8; 6] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];

struct Buds {
    /// Product ID bytes as sent in the 0x0103 reply: `b1 b2 b3` (ID string is `b3 b2 b1`).
    product: [u8; 3],
    battery: [(u8, bool); 3], // left, right, case
    /// The bit the buds *report* for the current mode (Buds 4: Off 3, Transparency 8, ANC 1, ...).
    noise_bit: u8,
    low_latency: bool,
    eq_id: u8,
    custom: Vec<CustomEq>,
    gestures: BTreeMap<(EarSide, Gesture), GestureAction>,
    wear: [u8; 2],
}

impl Buds {
    fn buds4() -> Buds {
        let hz = [62, 250, 1000, 4000, 8000, 16000];
        let mut gestures = BTreeMap::new();
        for side in EarSide::ALL {
            gestures.insert((side, Gesture::DoubleTap), GestureAction::PlayPause);
            gestures.insert((side, Gesture::TripleTap), GestureAction::Next);
            gestures.insert((side, Gesture::Hold), GestureAction::NoiseControl);
        }
        Buds {
            product: [0x14, 0x54, 0x06], // 065414
            battery: [(80, false), (82, false), (45, false)],
            noise_bit: 3, // Off
            low_latency: false,
            eq_id: 0,
            custom: vec![CustomEq {
                id: 3,
                name: "Custom".into(),
                min_gain: -6,
                max_gain: 6,
                bands: hz.iter().map(|&hz| EqBand { hz, gain: 0 }).collect(),
            }],
            gestures,
            wear: [3, 3],
        }
    }

    fn battery_payload(&self) -> Vec<u8> {
        self.battery_payload_with(0x00)
    }

    /// `lead` is the status byte of a reply, or the push type (1) of a battery push.
    fn battery_payload_with(&self, lead: u8) -> Vec<u8> {
        let mut p = vec![lead, 0x03];
        for (i, (level, charging)) in self.battery.iter().enumerate() {
            p.push(i as u8 + 1);
            p.push(level | if *charging { 0x80 } else { 0 });
        }
        p
    }

    fn noise_payload(&self, lead: u8) -> Vec<u8> {
        let mask = 1u16 << self.noise_bit;
        vec![lead, 0x01, 0x01, (mask & 0xFF) as u8, (mask >> 8) as u8]
    }

    fn eq_list_payload(&self) -> Vec<u8> {
        let mut p = vec![0x00, self.custom.len() as u8];
        for eq in &self.custom {
            let selected = u8::from(self.eq_id == eq.id);
            p.extend(
                crate::encode::set_custom_eq(eq)
                    .iter()
                    .enumerate()
                    .map(|(i, b)| if i == 0 { selected } else { *b }),
            );
        }
        p
    }

    fn hosts_payload(lead: u8) -> Vec<u8> {
        let mut p = vec![lead, 0x02];
        // The phone: connected, not this host.
        p.extend([0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0x00, 0x02, 0x00, 5]);
        p.extend(b"Phone");
        // This computer: connected, this host.
        p.extend([0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12, 0x00, 0x02, 0x01, 7]);
        p.extend(b"This PC");
        p
    }
}

/// What the world looks like to the controller, plus the knobs tests turn.
struct World {
    present: bool,
    connected: bool,
    address: [u8; 6],
    name: String,
    buds: Buds,
    open_hangs: bool,
    open_error: Option<String>,
    /// `open` succeeds but the link is already dead, so every write fails (BlueZ accepting a connect the
    /// buds immediately drop).
    open_dead: bool,
    drop_replies: bool,
    ignore: HashSet<u16>,
    noise_reply_delay_once: Option<Duration>,
    skew: Duration,
    page_connects: bool,
    page_count: u32,
    open_times: Vec<Instant>,
    connected_at: Option<Instant>,
    sent: Vec<(u16, Vec<u8>)>,
    sink: Option<LinkSink>,
    link_open: Option<Arc<Mutex<bool>>>,
    budget: Duration,
}

/// The simulated platform. Clones share the same world.
#[derive(Clone)]
pub struct FakePlatform {
    world: Arc<Mutex<World>>,
}

impl Default for FakePlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl FakePlatform {
    /// OnePlus Buds 4, paired and connected, noise mode Off.
    pub fn new() -> FakePlatform {
        FakePlatform {
            world: Arc::new(Mutex::new(World {
                present: true,
                connected: true,
                address: FAKE_ADDRESS,
                name: "OnePlus Buds 4".into(),
                buds: Buds::buds4(),
                open_hangs: false,
                open_error: None,
                open_dead: false,
                drop_replies: false,
                ignore: HashSet::new(),
                noise_reply_delay_once: None,
                skew: Duration::ZERO,
                page_connects: false,
                page_count: 0,
                open_times: Vec::new(),
                connected_at: None,
                sent: Vec::new(),
                sink: None,
                link_open: None,
                budget: Duration::from_secs(8),
            })),
        }
    }

    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.world.lock().unwrap()
    }

    // --- Knobs ---

    /// The buds are in the paired-devices list (false = not paired / unknown).
    pub fn set_present(&self, present: bool) {
        self.world().present = present;
    }

    /// Connected to this computer. Turning it off also kills an open control link, like the buds
    /// going into the case.
    pub fn set_connected(&self, connected: bool) {
        let mut w = self.world();
        w.connected = connected;
        if connected {
            w.connected_at = Some(Instant::now());
        } else {
            drop(w);
            self.close_link();
        }
    }

    /// The buds are out of the case but only joined the phone: paired, not connected here.
    pub fn phone_only(&self) {
        self.set_connected(false);
    }

    /// Paging brings the buds to this computer (like the real thing succeeding).
    pub fn set_page_connects(&self, yes: bool) {
        self.world().page_connects = yes;
    }

    /// Silently ignore all requests (the link looks dead).
    pub fn drop_replies(&self, yes: bool) {
        self.world().drop_replies = yes;
    }

    /// Never answer this command.
    pub fn ignore_command(&self, command: u16) {
        self.world().ignore.insert(command);
    }

    /// `open` never completes.
    pub fn hang_open(&self, yes: bool) {
        self.world().open_hangs = yes;
    }

    /// `open` fails with this message.
    pub fn fail_open(&self, message: Option<&str>) {
        self.world().open_error = message.map(str::to_string);
    }

    /// `open` succeeds, but the link it returns is already dead: every write fails.
    pub fn open_dead(&self, yes: bool) {
        self.world().open_dead = yes;
    }

    /// The remote side closes the control channel.
    pub fn close_link(&self) {
        let sink = {
            let mut w = self.world();
            if let Some(open) = w.link_open.take() {
                *open.lock().unwrap() = false;
            }
            w.sink.take()
        };
        if let Some(sink) = sink {
            sink.closed();
        }
    }

    /// Pretends the computer slept for `by`: the wall clock jumps forward.
    pub fn skew_clock(&self, by: Duration) {
        self.world().skew += by;
    }

    /// The next noise-query reply is delayed and carries the mode as it is *now* (a stale answer).
    pub fn delay_next_noise_reply(&self, by: Duration) {
        self.world().noise_reply_delay_once = Some(by);
    }

    /// Changes which buds this is (different address, name and product ID bytes `b1 b2 b3`), and
    /// drops the link.
    pub fn swap_device(&self, address: [u8; 6], name: &str, product: [u8; 3], noise_bit: u8) {
        {
            let mut w = self.world();
            w.address = address;
            w.name = name.to_string();
            w.buds.product = product;
            w.buds.noise_bit = noise_bit;
        }
        self.close_link();
    }

    /// Sets the mode the buds report, as if changed on the buds (long press).
    pub fn set_reported_noise_bit(&self, bit: u8) {
        self.world().buds.noise_bit = bit;
    }

    pub fn set_battery(&self, left: (u8, bool), right: (u8, bool), case: (u8, bool)) {
        self.world().buds.battery = [left, right, case];
    }

    /// Like `set_battery`, and the buds announce it with a push, as real ones do when a level changes.
    pub fn push_battery(&self, left: (u8, bool), right: (u8, bool), case: (u8, bool)) {
        let (sink, payload) = {
            let mut w = self.world();
            w.buds.battery = [left, right, case];
            (w.sink.clone(), w.buds.battery_payload_with(push::BATTERY))
        };
        if let Some(sink) = sink {
            sink.data(Frame::new(cmd::STATUS_PUSH, 0, payload).encode());
        }
    }

    /// The EQ preset changes on the buds themselves (stem gesture, phone app), and they announce it.
    pub fn push_eq(&self, id: u8) {
        let sink = {
            let mut w = self.world();
            w.buds.eq_id = id;
            w.sink.clone()
        };
        if let Some(sink) = sink {
            sink.data(Frame::new(cmd::EQ_PUSH, 0, vec![id]).encode());
        }
    }

    // --- Observation ---

    /// Every command received from the controller, in order.
    pub fn sent(&self) -> Vec<(u16, Vec<u8>)> {
        self.world().sent.clone()
    }

    pub fn clear_sent(&self) {
        self.world().sent.clear();
    }

    /// Commands received so far with this command number.
    pub fn count_sent(&self, command: u16) -> usize {
        self.world()
            .sent
            .iter()
            .filter(|(c, _)| *c == command)
            .count()
    }

    pub fn page_count(&self) -> u32 {
        self.world().page_count
    }

    pub fn open_count(&self) -> usize {
        self.world().open_times.len()
    }

    /// When each `open` call started.
    pub fn open_times(&self) -> Vec<Instant> {
        self.world().open_times.clone()
    }

    /// When the buds were last made connected.
    pub fn connected_at(&self) -> Option<Instant> {
        self.world().connected_at
    }

    pub fn reported_noise_bit(&self) -> u8 {
        self.world().buds.noise_bit
    }

    pub fn low_latency(&self) -> bool {
        self.world().buds.low_latency
    }
}

struct FakeLink {
    world: Arc<Mutex<World>>,
    sink: LinkSink,
    open: Arc<Mutex<bool>>,
    parser: Mutex<FrameParser>,
}

impl Link for FakeLink {
    fn channel(&self) -> Option<u8> {
        Some(15)
    }

    fn connect_hint(&self) -> Option<String> {
        Some("channel 15".into())
    }

    fn write(&self, bytes: &[u8]) -> io::Result<()> {
        if !*self.open.lock().unwrap() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "link closed"));
        }
        let frames = self.parser.lock().unwrap().feed(bytes);
        for frame in frames {
            let outputs = {
                let mut w = self.world.lock().unwrap();
                w.sent.push((frame.command, frame.payload.clone()));
                if w.drop_replies || w.ignore.contains(&frame.command) {
                    continue;
                }
                respond(&mut w, &frame)
            };
            for (delay, out) in outputs {
                let sink = self.sink.clone();
                let bytes = out.encode();
                if delay.is_zero() {
                    sink.data(bytes);
                } else {
                    tokio::spawn(async move {
                        tokio::time::sleep(delay).await;
                        sink.data(bytes);
                    });
                }
            }
        }
        Ok(())
    }

    fn close(&self) {
        *self.open.lock().unwrap() = false;
    }
}

/// What the fake buds do with one request: `(delay, frame)` pairs to send back.
fn respond(w: &mut World, req: &Frame) -> Vec<(Duration, Frame)> {
    let now = Duration::ZERO;
    let ack = |p: Vec<u8>| (now, Frame::new(reply(req.command), req.seq, p));
    let pushed = |p: Vec<u8>| (PUSH_DELAY, Frame::new(cmd::STATUS_PUSH, 0, p));
    let p = req.payload.as_slice();
    match req.command {
        cmd::HELLO => {
            // Capability bits 3, 5, 7, 8, 10 and 34.
            vec![ack(vec![0x00, 0xA8, 0x05, 0x00, 0x00, 0x04])]
        }
        cmd::PRODUCT_ID => {
            let [b1, b2, b3] = w.buds.product;
            vec![ack(vec![0x00, b1, b2, b3])]
        }
        cmd::BATTERY => vec![ack(w.buds.battery_payload())],
        cmd::WEAR_STATUS => vec![ack(vec![0x00, 0x02, 1, w.buds.wear[0], 2, w.buds.wear[1]])],
        cmd::NOISE_QUERY => {
            let payload = w.buds.noise_payload(0x00);
            let delay = w.noise_reply_delay_once.take().unwrap_or(now);
            vec![(delay, Frame::new(reply(req.command), req.seq, payload))]
        }
        cmd::SET_NOISE => {
            // `01 01 <mask>`: the lowest set bit is the mode; the buds report some modes with a
            // different bit than the one used to set them (Off 0 -> 3, Transparency 2 -> 8).
            let mask = p
                .iter()
                .skip(2)
                .enumerate()
                .fold(0u32, |m, (i, b)| m | (u32::from(*b) << (8 * i)));
            if mask != 0 {
                let set_bit = mask.trailing_zeros() as u8;
                w.buds.noise_bit = match set_bit {
                    0 => 3,
                    2 => 8,
                    other => other,
                };
            }
            vec![ack(vec![0x00]), pushed(w.buds.noise_payload(push::NOISE))]
        }
        cmd::MULTIPOINT_QUERY => vec![ack(Buds::hosts_payload(0x00))],
        cmd::SUBSCRIBE => vec![ack(vec![0x00])],
        cmd::GESTURE_QUERY => {
            let mut payload = vec![0x00, w.buds.gestures.len() as u8];
            for ((side, gesture), action) in &w.buds.gestures {
                payload.extend([side.wire(), 0x01, gesture.wire(), action.wire()]);
            }
            vec![ack(payload)]
        }
        cmd::SET_GESTURE => {
            if p.len() >= 5
                && let (Some(side), Some(gesture), Some(action)) = (
                    EarSide::from_wire(p[1]),
                    Gesture::from_wire(p[3]),
                    GestureAction::from_wire(p[4]),
                )
            {
                w.buds.gestures.insert((side, gesture), action);
            }
            vec![ack(vec![0x00])]
        }
        cmd::SWITCH_QUERY => vec![ack(vec![0x00, 0x01, 6, u8::from(w.buds.low_latency)])],
        cmd::SET_SWITCH => {
            if p.len() >= 2 && p[0] == 6 {
                w.buds.low_latency = p[1] != 0;
            }
            vec![
                ack(vec![0x00]),
                pushed(vec![push::GAME_MODE, u8::from(w.buds.low_latency)]),
            ]
        }
        cmd::EQ_QUERY => vec![ack(vec![0x00, w.buds.eq_id])],
        cmd::SET_EQ => {
            if let Some(id) = p.first() {
                w.buds.eq_id = *id;
            }
            vec![ack(vec![0x00])]
        }
        cmd::EQ_LIST_QUERY => vec![ack(w.buds.eq_list_payload())],
        cmd::SET_CUSTOM_EQ => {
            // `02 <min> <max> <id> <name len> <name> <count> (<hz lo> <hz hi> <gain>)*`
            let mut list = vec![0x00, 0x01];
            list.extend_from_slice(p);
            if let Some(eqs) = crate::parse::eq_list(&list) {
                for eq in eqs {
                    match w.buds.custom.iter_mut().find(|e| e.id == eq.id) {
                        Some(slot) => *slot = eq,
                        None => w.buds.custom.push(eq),
                    }
                }
            }
            vec![ack(vec![0x00])]
        }
        cmd::FIND_BUDS => vec![ack(vec![if p.first() == Some(&1) { 0x00 } else { 0x01 }])],
        _ => vec![],
    }
}

impl Platform for FakePlatform {
    async fn paired_devices(&self) -> Vec<BtDevice> {
        let w = self.world();
        if !w.present {
            return vec![];
        }
        vec![BtDevice {
            address: w.address,
            name: Some(w.name.clone()),
            connected: w.connected,
            has_control_service: true,
        }]
    }

    async fn open(
        &self,
        _device: &BtDevice,
        _hint: Option<String>,
        sink: LinkSink,
        log: LogFn,
    ) -> Result<Box<dyn Link>, OpenError> {
        let (hangs, error, connected, dead) = {
            let mut w = self.world();
            w.open_times.push(Instant::now());
            (w.open_hangs, w.open_error.clone(), w.connected, w.open_dead)
        };
        log(LogDirection::Info, "opening RFCOMM channel 15");
        if hangs {
            std::future::pending::<()>().await;
        }
        tokio::task::yield_now().await;
        if let Some(message) = error {
            return Err(OpenError(message));
        }
        if !connected {
            return Err(OpenError("buds are not connected".into()));
        }
        let open = Arc::new(Mutex::new(!dead));
        {
            let mut w = self.world();
            w.sink = Some(sink.clone());
            w.link_open = Some(open.clone());
        }
        Ok(Box::new(FakeLink {
            world: self.world.clone(),
            sink,
            open,
            parser: Mutex::new(FrameParser::new()),
        }))
    }

    async fn page(&self, _device: &BtDevice, _log: LogFn) -> Result<(), String> {
        let mut w = self.world();
        w.page_count += 1;
        if w.page_connects {
            w.connected = true;
            w.connected_at = Some(Instant::now());
        }
        Ok(())
    }

    fn connect_budget(&self) -> Duration {
        self.world().budget
    }

    fn host_label(&self) -> &'static str {
        "PC"
    }

    fn wall_time(&self) -> SystemTime {
        SystemTime::now() + self.world().skew
    }
}
