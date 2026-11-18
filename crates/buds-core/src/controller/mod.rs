//! The controller: owns the link to the earbuds and all state shown in the UI.
//!
//! It is a single-task actor. Everything that happens
//! (user actions, platform results, timers, received bytes) is a [`Msg`] in one inbox, handled one
//! at a time; the resulting [`Snapshot`] is published on a `watch` channel when it changes.

mod connect;
mod frames;
mod remember;
pub mod snapshot;
mod view;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub use snapshot::*;

use crate::consts::*;
use crate::encode;
use crate::packet_log::PacketLog;
use crate::platform::{BtDevice, Link, LogDirection, OpenError, Platform};
use crate::protocol::{Frame, FrameParser, cmd, to_hex};
use crate::settings::SettingsStore;
use crate::types::*;

/// What the UI and tray can ask for.
#[derive(Debug, Clone)]
pub enum Action {
    /// Begin connecting and start the periodic timers. Send once.
    Start,
    /// Switch noise mode. `level` only matters for ANC; `None` picks the last used / first level.
    SetMode {
        mode: NoiseMode,
        level: Option<AncLevel>,
    },
    CycleMode,
    /// Developer: set any bit of the noise-mode mask.
    SetNoiseBit(u8),
    SetLowLatency(bool),
    SetEq(u8),
    /// New gains for the active custom EQ's bands.
    SetCustomEq(Vec<i32>),
    FindBuds(bool),
    SetGesture {
        side: EarSide,
        gesture: Gesture,
        action: GestureAction,
    },
    Reconnect,
    Refresh,
    QueryHosts,
    /// Developer: send an arbitrary command.
    SendRaw {
        command: u16,
        payload: Vec<u8>,
    },
    /// Developer: send every 0x0101-0x0120 query, spaced out.
    ScanQueries,
    /// Stop the controller task and close the link.
    Shutdown,
}

/// Things that aren't state: raw frames (developer tooling) and multipoint list changes.
#[derive(Debug, Clone)]
pub enum Event {
    Frame(Frame),
    Hosts(Vec<Host>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PollPurpose {
    /// Periodic device poll: detects buds coming and going.
    Health,
    /// Poll made right before a connection attempt.
    Connect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TimerKind {
    Retry,
    Poll,
    Health,
    ConnectingWatchdog,
    ReplyWatchdog,
    ApplyQueuedMode,
    FindStop,
    Resume,
    /// An EQ change was not confirmed in time.
    EqWatchdog,
    /// Read the EQ back once after the buds acknowledged a change.
    EqRecheck,
}

pub(crate) enum Msg {
    Action(Action),
    Devices {
        purpose: PollPurpose,
        devices: Vec<BtDevice>,
    },
    LinkOpened {
        attempt: u64,
        link_id: u64,
        result: Result<Box<dyn Link>, OpenError>,
    },
    Data {
        link: u64,
        bytes: Vec<u8>,
    },
    LinkClosed {
        link: u64,
    },
    PageDone(Result<(), String>),
    Timer(TimerKind, u64),
}

/// Cheap, cloneable remote control for a running [`Controller`].
#[derive(Clone)]
pub struct ControllerHandle {
    tx: mpsc::UnboundedSender<Msg>,
    state: watch::Receiver<Snapshot>,
    events: broadcast::Sender<Event>,
    log: PacketLog,
    settings: SettingsStore,
}

impl ControllerHandle {
    pub fn send(&self, action: Action) {
        let _ = self.tx.send(Msg::Action(action));
    }

    pub fn start(&self) {
        self.send(Action::Start);
    }
    pub fn set_mode(&self, mode: NoiseMode, level: Option<AncLevel>) {
        self.send(Action::SetMode { mode, level });
    }
    pub fn cycle_mode(&self) {
        self.send(Action::CycleMode);
    }
    pub fn set_noise_bit(&self, bit: u8) {
        self.send(Action::SetNoiseBit(bit));
    }
    pub fn set_low_latency(&self, on: bool) {
        self.send(Action::SetLowLatency(on));
    }
    pub fn set_eq(&self, id: u8) {
        self.send(Action::SetEq(id));
    }
    pub fn set_custom_eq(&self, gains: Vec<i32>) {
        self.send(Action::SetCustomEq(gains));
    }
    pub fn find_buds(&self, start: bool) {
        self.send(Action::FindBuds(start));
    }
    pub fn set_gesture(&self, side: EarSide, gesture: Gesture, action: GestureAction) {
        self.send(Action::SetGesture {
            side,
            gesture,
            action,
        });
    }
    pub fn reconnect(&self) {
        self.send(Action::Reconnect);
    }
    pub fn refresh(&self) {
        self.send(Action::Refresh);
    }
    pub fn query_hosts(&self) {
        self.send(Action::QueryHosts);
    }
    pub fn send_raw(&self, command: u16, payload: Vec<u8>) {
        self.send(Action::SendRaw { command, payload });
    }
    pub fn scan_queries(&self) {
        self.send(Action::ScanQueries);
    }
    pub fn shutdown(&self) {
        self.send(Action::Shutdown);
    }

    /// The latest published state.
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().clone()
    }
    /// A receiver that wakes whenever the state changes.
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.state.clone()
    }
    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    pub fn log(&self) -> &PacketLog {
        &self.log
    }
    pub fn settings(&self) -> &SettingsStore {
        &self.settings
    }
}

pub struct Controller<P: Platform> {
    platform: Arc<P>,
    settings: SettingsStore,
    log: PacketLog,
    tx: mpsc::UnboundedSender<Msg>,
    rx: mpsc::UnboundedReceiver<Msg>,
    state_tx: watch::Sender<Snapshot>,
    events: broadcast::Sender<Event>,

    // --- State shown in the UI ---
    link: LinkState,
    device_name: String,
    product_id: Option<String>,
    mode: Option<NoiseMode>,
    level: Option<AncLevel>,
    noise_bit: Option<u8>,
    left: Option<Battery>,
    right: Option<Battery>,
    case_battery: Option<Battery>,
    game_mode: Option<bool>,
    eq_id: Option<u8>,
    /// EQ preset we asked for and the buds haven't confirmed yet. While set, other readings are stale.
    pending_eq: Option<u8>,
    /// What the next EQ re-read should also fetch: the preset list (custom EQ changes).
    eq_reread_list: bool,
    custom_eqs: Vec<CustomEq>,
    gestures: BTreeMap<EarSide, BTreeMap<Gesture, GestureAction>>,
    finding_buds: bool,
    hosts: Vec<Host>,
    wear: BTreeMap<u8, Wear>,
    channel_id: Option<u8>,
    address: Option<String>,
    capabilities: Option<BTreeSet<u16>>,

    // --- Connection machinery ---
    device: Option<BtDevice>,
    devices: Vec<BtDevice>,
    /// Addresses of buds seen connected by the last health poll (`None` before the first poll).
    prev_connected: Option<HashSet<[u8; 6]>>,
    connected_since: HashMap<[u8; 6], Instant>,
    link_handle: Option<Box<dyn Link>>,
    open_task: Option<JoinHandle<()>>,
    /// Bumped on every open and on `close_channel`; a result for an old attempt is discarded.
    attempt: u64,
    next_link_id: u64,
    /// Id of the open link; data and close events for any other id are dropped.
    current_link: u64,
    parser: FrameParser,
    seq: u8,
    retry_count: usize,
    /// Control-channel opens that failed in a row, and when the next one may start. The OS can keep
    /// reporting buds as connected after they left, so every open would just page buds that aren't
    /// there: after two failures wait 6 s, 12 s, ... up to 5 min until they really connect or a
    /// reconnect is asked for.
    failed_opens: u32,
    next_open: Option<Instant>,
    last_received: Option<Instant>,
    reply_sent: Option<Instant>,
    page_in_flight: bool,
    last_page: Option<Instant>,
    last_wall: SystemTime,
    timer_gen: HashMap<TimerKind, u64>,
    /// `Action::Start` has been handled.
    started: bool,
    /// How this link was reached; saved as the connection hint when the buds first answer.
    pending_hint: Option<String>,

    // --- Remembered settings / queued work ---
    restore_noise: bool,
    restore_low_latency: bool,
    /// The buds have reported their noise mode / low-latency switch on the *current* link. The
    /// values kept from before a reconnect are stale and must not be compared with the remembered
    /// settings (the Swift code could consume the restore on a stale value).
    noise_fresh: bool,
    switch_fresh: bool,
    /// Noise queries still in flight. Answers to queries sent before the latest mode change describe
    /// the old mode, so they're dropped instead of flipping the UI back for a moment.
    outstanding_noise_queries: BTreeSet<u8>,
    stale_noise_queries: BTreeSet<u8>,
    /// A mode change requested while the link was down; sent as soon as it comes up.
    queued_mode: Option<(NoiseMode, Option<AncLevel>)>,
}

/// Creates a controller and its handle. Run the controller with [`Controller::run`] (or use
/// [`spawn`]).
impl<P: Platform> Controller<P> {
    pub fn new(
        platform: Arc<P>,
        settings: SettingsStore,
        log: PacketLog,
    ) -> (Controller<P>, ControllerHandle) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (state_tx, state_rx) = watch::channel(Snapshot::initial(platform.host_label()));
        let (events, _) = broadcast::channel(256);
        let last_wall = platform.wall_time();
        let controller = Controller {
            platform,
            settings: settings.clone(),
            log: log.clone(),
            tx: tx.clone(),
            rx,
            state_tx,
            events: events.clone(),
            link: LinkState::Searching,
            device_name: "Earbuds".into(),
            product_id: None,
            mode: None,
            level: None,
            noise_bit: None,
            left: None,
            right: None,
            case_battery: None,
            game_mode: None,
            eq_id: None,
            pending_eq: None,
            eq_reread_list: false,
            custom_eqs: Vec::new(),
            gestures: BTreeMap::new(),
            finding_buds: false,
            hosts: Vec::new(),
            wear: BTreeMap::new(),
            channel_id: None,
            address: None,
            capabilities: None,
            device: None,
            devices: Vec::new(),
            prev_connected: None,
            connected_since: HashMap::new(),
            link_handle: None,
            open_task: None,
            attempt: 0,
            next_link_id: 0,
            current_link: 0,
            parser: FrameParser::new(),
            seq: 0,
            retry_count: 0,
            failed_opens: 0,
            next_open: None,
            last_received: None,
            reply_sent: None,
            page_in_flight: false,
            last_page: None,
            last_wall,
            timer_gen: HashMap::new(),
            started: false,
            pending_hint: None,
            restore_noise: false,
            restore_low_latency: false,
            noise_fresh: false,
            switch_fresh: false,
            outstanding_noise_queries: BTreeSet::new(),
            stale_noise_queries: BTreeSet::new(),
            queued_mode: None,
        };
        let handle = ControllerHandle {
            tx,
            state: state_rx,
            events,
            log,
            settings,
        };
        (controller, handle)
    }

    /// Processes messages until [`Action::Shutdown`].
    pub async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            let keep_going = self.handle(msg);
            self.publish();
            if !keep_going {
                break;
            }
        }
        self.close_channel();
        self.publish();
    }

    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Action(action) => return self.action(action),
            Msg::Devices {
                purpose: PollPurpose::Connect,
                devices,
            } => self.connect_with(devices),
            Msg::Devices {
                purpose: PollPurpose::Health,
                devices,
            } => self.health(devices),
            Msg::LinkOpened {
                attempt,
                link_id,
                result,
            } => self.link_opened(attempt, link_id, result),
            Msg::Data { link, bytes } => {
                if link == self.current_link && self.link_handle.is_some() {
                    self.received(&bytes);
                }
            }
            Msg::LinkClosed { link } => {
                if link == self.current_link && self.link_handle.is_some() {
                    self.link_closed();
                }
            }
            Msg::PageDone(result) => self.page_done(result),
            Msg::Timer(kind, generation) => {
                if self.timer_gen.get(&kind) == Some(&generation) {
                    self.timer(kind);
                }
            }
        }
        true
    }

    fn action(&mut self, action: Action) -> bool {
        match action {
            Action::Start => self.start(),
            Action::SetMode { mode, level } => self.set_mode(mode, level),
            Action::CycleMode => self.cycle_mode(),
            Action::SetNoiseBit(bit) => self.set_noise_bit(bit),
            Action::SetLowLatency(on) => self.set_game_mode(on),
            Action::SetEq(id) => self.set_eq(id),
            Action::SetCustomEq(gains) => self.set_custom_eq(&gains),
            Action::FindBuds(start) => self.find_buds(start),
            Action::SetGesture {
                side,
                gesture,
                action,
            } => self.set_gesture(action, gesture, side),
            Action::Reconnect => self.reconnect(),
            Action::Refresh => self.refresh(),
            Action::QueryHosts => self.send(cmd::MULTIPOINT_QUERY, vec![]),
            Action::SendRaw { command, payload } => self.send(command, payload),
            Action::ScanQueries => self.scan_queries(),
            Action::Shutdown => return false,
        }
        true
    }

    // Timers

    /// (Re)starts a one-shot timer; scheduling a kind again cancels the previous one.
    fn schedule(&mut self, kind: TimerKind, after: Duration) {
        let generation = self.bump(kind);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(after).await;
            let _ = tx.send(Msg::Timer(kind, generation));
        });
    }

    fn cancel(&mut self, kind: TimerKind) {
        self.bump(kind);
    }

    fn bump(&mut self, kind: TimerKind) -> u64 {
        let generation = self.timer_gen.entry(kind).or_insert(0);
        *generation += 1;
        *generation
    }

    fn timer(&mut self, kind: TimerKind) {
        match kind {
            TimerKind::Retry => self.connect(),
            TimerKind::Poll => {
                self.refresh();
                self.schedule(TimerKind::Poll, POLL_INTERVAL);
            }
            TimerKind::Health => {
                self.health_tick();
                self.schedule(TimerKind::Health, HEALTH_INTERVAL);
            }
            TimerKind::ConnectingWatchdog => {
                // Last line of defence: no attempt may sit in "Connecting…" for long.
                if self.link == LinkState::Connecting {
                    self.info("stuck connecting, starting over");
                    self.close_channel();
                    self.open_failed();
                    self.retry_count = 0;
                    self.schedule_retry();
                }
            }
            TimerKind::ReplyWatchdog => {
                let silent = match (self.last_received, self.reply_sent) {
                    (Some(received), Some(sent)) => received < sent,
                    (None, Some(_)) => true,
                    _ => false,
                };
                if self.link == LinkState::Connected && silent {
                    self.info("no reply from buds, reconnecting");
                    // Whatever was remembered did not work this time; start the next walk fresh
                    // instead of insisting on the same service forever.
                    if let Some(address) = self.address.clone() {
                        self.settings.update(|s| {
                            s.connect_hints.remove(&address);
                        });
                    }
                    self.reconnect();
                }
            }
            TimerKind::ApplyQueuedMode => self.apply_queued_mode(),
            TimerKind::FindStop => self.find_buds(false),
            TimerKind::Resume => {
                if self.link == LinkState::Connected {
                    self.refresh();
                } else {
                    self.restart();
                }
            }
            TimerKind::EqWatchdog => {
                // No confirmation: read back and take whatever the buds report.
                self.pending_eq = None;
                self.reread_eq();
            }
            TimerKind::EqRecheck => {
                if self.link == LinkState::Connected {
                    if self.pending_eq.is_some() {
                        self.send(cmd::EQ_QUERY, vec![]);
                    }
                    if self.eq_reread_list {
                        self.send(cmd::EQ_LIST_QUERY, encode::EQ_LIST_QUERY.to_vec());
                    }
                }
            }
        }
    }

    // Public operations

    fn start(&mut self) {
        if self.started {
            return; // a second client (budsctl asking the running app) must not restart anything
        }
        self.started = true;
        self.connect();
        self.schedule(TimerKind::Poll, POLL_INTERVAL);
        self.schedule(TimerKind::Health, HEALTH_INTERVAL);
    }

    /// Asked for (or the link just broke): also skips any wait from earlier failed opens.
    pub(crate) fn reconnect(&mut self) {
        self.failed_opens = 0;
        self.restart();
    }

    /// One try straight away; if it fails, the wait from earlier failures carries on growing.
    fn restart(&mut self) {
        self.close_channel();
        self.retry_count = 0;
        self.next_open = None;
        self.connect();
    }

    fn refresh(&mut self) {
        if self.link != LinkState::Connected {
            return;
        }
        // Pick up a rename made in the OS Bluetooth settings while connected.
        if let Some(device) = &self.device
            && let Some(current) = self.devices.iter().find(|d| d.address == device.address)
            && let Some(name) = &current.name
            && *name != self.device_name
        {
            self.device_name = name.clone();
        }
        self.send(cmd::BATTERY, vec![]);
        self.query_noise();
        self.expect_reply();
    }

    /// Switches immediately: the UI updates before the buds answer, and their acknowledgement
    /// triggers a single confirming query.
    fn set_mode(&mut self, mode: NoiseMode, level: Option<AncLevel>) {
        if self.link != LinkState::Connected {
            self.queued_mode = Some((mode, level));
            if self.link == LinkState::Searching {
                self.reconnect();
            }
            return;
        }
        // Plain "ANC" uses the last level picked, so the level row always shows a selection;
        // models without levels use their plain ANC mode.
        let mut level = level;
        let available = self.available_levels();
        if mode == NoiseMode::Anc && level.is_none() && !available.is_empty() {
            level = self
                .level
                .or_else(|| self.settings.read(|s| s.last_anc_level));
            if !level.is_some_and(|l| available.contains(&l)) {
                level = available.first().copied();
            }
        }
        let wanted_type = match mode {
            NoiseMode::Off => mode_type::OFF,
            NoiseMode::Transparency => mode_type::TRANSPARENCY,
            NoiseMode::Anc => level.map_or(mode_type::ANC, AncLevel::mode_type),
        };
        let Some(bit) = self.model().bit_for_type(wanted_type) else {
            let pid = self
                .product_id
                .clone()
                .unwrap_or_else(|| "unknown model".into());
            self.info(&format!("mode {} not supported by {pid}", mode.key()));
            return;
        };
        self.mode = Some(mode);
        self.set_level(if mode == NoiseMode::Anc { level } else { None });
        self.remember(|r| {
            r.mode = Some(mode);
            r.level = level;
        });
        self.set_noise_bit(bit);
    }

    fn cycle_mode(&mut self) {
        let available = self.available_modes();
        let order: Vec<NoiseMode> = [NoiseMode::Anc, NoiseMode::Transparency, NoiseMode::Off]
            .into_iter()
            .filter(|m| available.contains(m))
            .collect();
        if order.is_empty() {
            return;
        }
        let current = order
            .iter()
            .position(|m| Some(*m) == self.mode.or(Some(NoiseMode::Off)))
            .unwrap_or(order.len() - 1);
        self.set_mode(order[(current + 1) % order.len()], None);
    }

    /// Developer: set any bit of the noise-mode mask, including ones the UI doesn't name.
    fn set_noise_bit(&mut self, bit: u8) {
        let payload = encode::set_noise(bit);
        self.stale_noise_queries
            .extend(self.outstanding_noise_queries.iter().copied());
        self.noise_bit = Some(bit);
        if let Some(t) = self.model().reported_type(bit) {
            self.mode = Some(mode_type::mode(t));
            if let Some(level) = AncLevel::from_mode_type(t) {
                self.set_level(Some(level));
            }
        }
        self.send(cmd::SET_NOISE, payload);
        self.expect_reply();
    }

    /// Low-latency "game mode" switch (misc config 0x06).
    fn set_game_mode(&mut self, on: bool) {
        self.game_mode = Some(on);
        self.remember(|r| r.low_latency = Some(on));
        self.send(
            cmd::SET_SWITCH,
            encode::set_switch(crate::protocol::feature::LOW_LATENCY, on),
        );
        self.expect_reply();
    }

    /// Waits for the buds to confirm preset `id`: a 0x0504 push or a matching read-back clears it;
    /// otherwise it is re-read after [`EQ_CONFIRM_TIMEOUT`] and the buds' answer taken. Until then,
    /// readings of another preset are stale (the DSP is still switching) and ignored.
    fn expect_eq(&mut self, id: u8, list: bool) {
        self.pending_eq = Some(id);
        self.eq_reread_list = list;
        self.cancel(TimerKind::EqRecheck);
        self.schedule(TimerKind::EqWatchdog, EQ_CONFIRM_TIMEOUT);
    }

    fn reread_eq(&mut self) {
        self.send(cmd::EQ_QUERY, vec![]);
        if self.eq_reread_list {
            self.send(cmd::EQ_LIST_QUERY, encode::EQ_LIST_QUERY.to_vec());
        }
    }

    /// The buds confirmed preset `id` (a push or a matching read-back). After a custom EQ edit the
    /// recheck stays scheduled: the push arrives a few ms after the acknowledgement, and the custom EQ
    /// list still has to be read back.
    pub(super) fn confirm_eq(&mut self, id: u8) {
        self.pending_eq = None;
        self.cancel(TimerKind::EqWatchdog);
        if !self.eq_reread_list {
            self.cancel(TimerKind::EqRecheck);
        }
        self.eq_id = Some(id);
    }

    /// Drops any EQ change still waiting for confirmation.
    pub(super) fn clear_pending_eq(&mut self) {
        self.pending_eq = None;
        self.cancel(TimerKind::EqWatchdog);
        self.cancel(TimerKind::EqRecheck);
    }

    fn set_eq(&mut self, id: u8) {
        self.eq_id = Some(id);
        self.expect_eq(id, false);
        self.send(cmd::SET_EQ, encode::set_eq(id));
        self.expect_reply();
    }

    /// Rewrites the active custom EQ's band gains (HeyMelody setEqInfo, action 2 = apply existing).
    fn set_custom_eq(&mut self, gains: &[i32]) {
        let active = self
            .custom_eqs
            .iter()
            .position(|e| Some(e.id) == self.eq_id)
            .or(if self.custom_eqs.is_empty() {
                None
            } else {
                Some(0)
            });
        let Some(index) = active else { return };
        let mut eq = self.custom_eqs[index].clone();
        encode::apply_gains(&mut eq, gains);
        let payload = encode::set_custom_eq(&eq);
        self.eq_id = Some(eq.id);
        self.expect_eq(eq.id, true);
        self.custom_eqs[index] = eq;
        self.send(cmd::SET_CUSTOM_EQ, payload);
        self.expect_reply();
    }

    /// Plays the buds' locator sound. It's loud; it stops by itself after 30 s.
    fn find_buds(&mut self, start: bool) {
        self.finding_buds = start;
        self.send(cmd::FIND_BUDS, encode::find_buds(start));
        self.cancel(TimerKind::FindStop);
        if start {
            self.schedule(TimerKind::FindStop, FIND_AUTO_STOP);
        }
    }

    fn set_gesture(&mut self, action: GestureAction, gesture: Gesture, side: EarSide) {
        self.gestures
            .entry(side)
            .or_default()
            .insert(gesture, action);
        self.send(cmd::SET_GESTURE, encode::set_gesture(side, gesture, action));
        self.expect_reply();
    }

    fn scan_queries(&mut self) {
        for (i, command) in (0x0101u16..=0x0120).enumerate() {
            let tx = self.tx.clone();
            let delay = SCAN_STEP * i as u32;
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                let _ = tx.send(Msg::Action(Action::SendRaw {
                    command,
                    payload: vec![],
                }));
            });
        }
    }

    // Sending

    /// Frames a request and writes it. A failed write reconnects.
    fn send(&mut self, command: u16, payload: Vec<u8>) {
        if self.link_handle.is_none() {
            return;
        }
        self.seq = self.seq.wrapping_add(1);
        let bytes = Frame::new(command, self.seq, payload).encode();
        self.log.append(LogDirection::Sent, &to_hex(&bytes));
        let result = self.link_handle.as_ref().map(|l| l.write(&bytes));
        if let Some(Err(error)) = result {
            // Same as the remote closing the channel: close it (so the rest of a burst of requests is
            // dropped instead of each failing) and retry on the timer. An immediate reconnect here looped
            // every few ms when a link opened already dead (BlueZ accepts the RFCOMM connect, the buds drop it).
            self.info(&format!("write failed: {error}, reconnecting"));
            self.link_closed();
        }
    }

    fn query_noise(&mut self) {
        self.send(cmd::NOISE_QUERY, encode::NOISE_QUERY.to_vec());
        self.outstanding_noise_queries.insert(self.seq);
    }

    /// If the buds don't answer a request within 3 s the link is dead (sleep, out of range without
    /// a close callback…), so start over.
    fn expect_reply(&mut self) {
        self.reply_sent = Some(Instant::now());
        self.schedule(TimerKind::ReplyWatchdog, REPLY_WATCHDOG);
    }

    // Small helpers

    fn info(&self, text: &str) {
        self.log.append(LogDirection::Info, text);
    }

    /// Sets the ANC level and remembers a non-empty one as the "last level".
    fn set_level(&mut self, level: Option<AncLevel>) {
        self.level = level;
        if let Some(level) = level
            && self.settings.read(|s| s.last_anc_level) != Some(level)
        {
            self.settings.update(|s| s.last_anc_level = Some(level));
        }
    }
}

/// Creates a controller and runs it on the current tokio runtime.
pub fn spawn<P: Platform>(
    platform: Arc<P>,
    settings: SettingsStore,
    log: PacketLog,
) -> ControllerHandle {
    let (controller, handle) = Controller::new(platform, settings, log);
    tokio::spawn(controller.run());
    handle
}
