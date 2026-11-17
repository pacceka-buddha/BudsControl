//! Reacting to what the buds send: replies and pushes. The byte layouts are decoded by `crate::parse`.

use tokio::time::Instant;

use super::*;
use crate::parse::{self, NoiseReport};
use crate::protocol::{feature, push, reply};

impl<P: Platform> Controller<P> {
    pub(super) fn received(&mut self, bytes: &[u8]) {
        self.last_received = Some(Instant::now());
        self.log.append(LogDirection::Received, &to_hex(bytes));
        let frames = self.parser.feed(bytes);
        // Remembered only once the buds send a well-formed frame. Any bytes at all are not enough:
        // the serial port service on the Buds 4 accepts a connection and sends unreadable data, and
        // remembering it would make every later start use it and never reach the control service.
        if !frames.is_empty()
            && let (Some(hint), Some(address)) = (self.pending_hint.take(), self.address.clone())
        {
            self.settings.update(|s| {
                s.connect_hints.insert(address, hint);
            });
        }
        for frame in frames {
            self.handle_frame(frame);
        }
    }

    fn handle_frame(&mut self, frame: Frame) {
        let _ = self.events.send(Event::Frame(frame.clone()));
        let p = frame.payload.as_slice();
        match frame.command {
            c if c == reply(cmd::PRODUCT_ID) => {
                if let Some(pid) = parse::product_id(p) {
                    self.product_id = Some(pid);
                    // Prefer the catalog's name when the OS gave us none, or only a generic one.
                    let model_name = self.model().name;
                    let generic =
                        self.device_name == "Earbuds" || self.device_name == "OnePlus Buds";
                    if !model_name.is_empty()
                        && (self.device.as_ref().is_none_or(|d| d.name.is_none()) || generic)
                    {
                        self.device_name = model_name.to_string();
                    }
                    self.apply_queued_mode();
                }
            }
            c if c == reply(cmd::HELLO) => {
                if let Some(bits) = parse::capabilities(p) {
                    self.capabilities = Some(bits);
                }
            }
            c if c == reply(cmd::BATTERY) => self.apply_battery(p),
            c if c == reply(cmd::NOISE_QUERY) => {
                self.outstanding_noise_queries.remove(&frame.seq);
                if self.stale_noise_queries.remove(&frame.seq) {
                    return;
                }
                self.apply_noise(p);
            }
            c if c == reply(cmd::SET_NOISE) => {
                // Acknowledged: read back which level the buds actually picked.
                if p.first().is_some_and(|s| *s != 0) {
                    self.info(&format!("mode change rejected: {}", to_hex(p)));
                }
                self.query_noise();
            }
            c if c == reply(cmd::SET_SWITCH) => {
                // An accepted switch is confirmed by a push about half a second later; reading it
                // back straight away still returns the old value and flicks the toggle. Re-read
                // only on refusal.
                if p.first().is_some_and(|s| *s != 0) {
                    self.info(&format!("switch rejected: {}", to_hex(p)));
                    self.send(cmd::SWITCH_QUERY, encode::LOW_LATENCY_QUERY.to_vec());
                }
            }
            c if c == reply(cmd::SWITCH_QUERY) => {
                if let Some(switches) = parse::switches(p) {
                    for (id, on) in switches {
                        if id == feature::LOW_LATENCY {
                            self.game_mode = Some(on);
                            self.switch_fresh = true;
                        }
                    }
                    self.restore_settings();
                }
            }
            c if c == reply(cmd::EQ_QUERY) => {
                if let Some(reported) = parse::eq_selected(p) {
                    match self.pending_eq {
                        // The buds confirmed what we asked for.
                        Some(pending) if pending == reported => {
                            self.clear_pending_eq();
                            self.eq_id = Some(reported);
                        }
                        // Stale reading while the DSP is switching: ignore it.
                        Some(_) => {}
                        None => self.eq_id = Some(reported),
                    }
                }
            }
            cmd::EQ_PUSH => {
                if let Some(&id) = p.first() {
                    self.info(&format!("EQ push: preset {id}"));
                    match self.pending_eq {
                        // A late push from an earlier selection is dropped.
                        Some(pending) if pending == id => {
                            self.clear_pending_eq();
                            self.eq_id = Some(id);
                        }
                        Some(_) => {}
                        // Changed from outside (stem gesture, phone app).
                        None => self.eq_id = Some(id),
                    }
                }
            }
            c if c == reply(cmd::EQ_LIST_QUERY) => {
                if let Some(list) = parse::eq_list(p)
                    && list != self.custom_eqs
                {
                    self.custom_eqs = list;
                }
            }
            c if c == reply(cmd::SET_EQ) || c == reply(cmd::SET_CUSTOM_EQ) => {
                let custom = c == reply(cmd::SET_CUSTOM_EQ);
                if p.first().is_some_and(|s| *s != 0) {
                    let kind = if custom { "custom " } else { "" };
                    self.info(&format!("{kind}EQ change rejected: {}", to_hex(p)));
                    // Nothing to wait for: read back what the buds actually have.
                    self.clear_pending_eq();
                    self.send(cmd::EQ_QUERY, vec![]);
                    if custom {
                        self.send(cmd::EQ_LIST_QUERY, encode::EQ_LIST_QUERY.to_vec());
                    }
                } else if self.pending_eq.is_some() || custom {
                    // Accepted: check once more shortly, in case no push or confirmation follows.
                    self.eq_reread_list = custom;
                    self.schedule(TimerKind::EqRecheck, EQ_RECHECK_DELAY);
                }
            }
            c if c == reply(cmd::GESTURE_QUERY) => {
                if let Some(list) = parse::gestures(p) {
                    self.gestures.clear();
                    for (side, gesture, action) in list {
                        self.gestures
                            .entry(side)
                            .or_default()
                            .insert(gesture, action);
                    }
                }
            }
            c if c == reply(cmd::SET_GESTURE) => {
                if p.first().is_some_and(|s| *s != 0) {
                    self.info(&format!("gesture change rejected: {}", to_hex(p)));
                }
                self.send(cmd::GESTURE_QUERY, vec![]);
            }
            c if c == reply(cmd::FIND_BUDS) => {
                // Status 0 accepts a start; the buds answer a stop with 1, so only a start can fail.
                if self.finding_buds && p.first().is_some_and(|s| *s != 0) {
                    self.info(&format!("find earbuds rejected: {}", to_hex(p)));
                    self.finding_buds = false;
                }
            }
            c if c == reply(cmd::MULTIPOINT_QUERY) => self.apply_hosts(p),
            c if c == reply(cmd::MULTIPOINT_SET) => {
                if p.first().is_some_and(|s| *s != 0) {
                    self.info(&format!("multipoint action rejected: {}", to_hex(p)));
                }
            }
            c if c == reply(cmd::WEAR_STATUS) => self.apply_wear(p),
            cmd::STATUS_PUSH => self.handle_push(p),
            _ => {}
        }
    }

    /// 0x0204 pushes start with the subscription type; the rest is laid out like the matching reply.
    fn handle_push(&mut self, p: &[u8]) {
        match p.first() {
            Some(&push::BATTERY) => self.apply_battery(p),
            Some(&push::WEAR) => self.apply_wear(p),
            Some(&push::NOISE) => self.apply_noise(p),
            Some(&push::GAME_MODE) if p.len() >= 2 => {
                self.game_mode = Some(p[1] != 0);
                self.switch_fresh = true;
            }
            Some(&push::HOSTS) => self.apply_hosts(p),
            _ => self.refresh(),
        }
    }

    fn apply_battery(&mut self, p: &[u8]) {
        if let Some(update) = parse::battery(p) {
            if update.left.is_some() {
                self.left = update.left;
            }
            if update.right.is_some() {
                self.right = update.right;
            }
            if let Some(case) = update.case {
                self.case_battery = case;
            }
        }
    }

    fn apply_wear(&mut self, p: &[u8]) {
        if let Some(entries) = parse::wear(p) {
            for (side, wear) in entries {
                match wear {
                    Some(w) => {
                        self.wear.insert(side, w);
                    }
                    None => {
                        self.wear.remove(&side);
                    }
                }
            }
        }
    }

    fn apply_hosts(&mut self, p: &[u8]) {
        if let Some(hosts) = parse::hosts(p) {
            self.hosts = hosts.clone();
            let _ = self.events.send(Event::Hosts(hosts));
        }
    }

    /// Payload contains `01 01 <mask lo> [<mask hi>]` where the lowest set bit is the active mode.
    fn apply_noise(&mut self, p: &[u8]) {
        match parse::noise_report(self.model(), p) {
            NoiseReport::None => {}
            NoiseReport::UnknownBit(bit) => {
                let pid = self
                    .product_id
                    .clone()
                    .unwrap_or_else(|| "unknown model".into());
                self.info(&format!("unknown noise bit {bit} for {pid}"));
            }
            NoiseReport::Known {
                bit, mode, level, ..
            } => {
                self.noise_bit = Some(bit);
                self.noise_fresh = true;
                self.mode = Some(mode);
                let level = level.or(if mode == NoiseMode::Anc {
                    self.level
                } else {
                    None
                });
                self.set_level(level);
                self.restore_settings();
            }
        }
    }

    pub(super) fn apply_queued_mode(&mut self) {
        if self.link != LinkState::Connected {
            return;
        }
        if let Some((mode, level)) = self.queued_mode.take() {
            self.set_mode(mode, level);
        }
    }
}
