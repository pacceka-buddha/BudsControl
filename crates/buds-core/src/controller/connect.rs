//! Finding the buds, opening and closing the control channel, retries, paging and presence
//! tracking.
//!
//! Instead of OS notifications (device connected / disconnected, wake from sleep) this polls: every
//! health tick lists the paired devices and diffs them.

use std::collections::HashSet;
use std::time::Duration;

use tokio::time::Instant;

use super::*;
use crate::platform::LinkSink;

impl<P: Platform> Controller<P> {
    /// Either they advertise the HeyMelody service (which also catches renamed buds), or their name
    /// carries the brand, or we have talked to this address before.
    pub(super) fn looks_like_buds(&self, device: &BtDevice) -> bool {
        if device.has_control_service {
            return true;
        }
        if device
            .name
            .as_deref()
            .is_some_and(crate::platform::name_looks_like_buds)
        {
            return true;
        }
        let address = device.address_string();
        self.settings.read(|s| s.known_buds.contains(&address))
    }

    /// Connected buds, preferring ones that advertise the HeyMelody control service.
    fn find_buds_in(&self, devices: &[BtDevice]) -> Option<BtDevice> {
        let connected: Vec<&BtDevice> = devices
            .iter()
            .filter(|d| d.connected && self.looks_like_buds(d))
            .collect();
        connected
            .iter()
            .find(|d| d.has_control_service)
            .or(connected.first())
            .map(|d| (*d).clone())
    }

    /// The buds this computer last talked to, or any paired buds; connected or not.
    fn paired_buds(&self, devices: &[BtDevice]) -> Option<BtDevice> {
        let paired: Vec<&BtDevice> = devices.iter().filter(|d| self.looks_like_buds(d)).collect();
        let last = self.settings.read(|s| s.last_buds_address.clone());
        paired
            .iter()
            .find(|d| Some(d.address_string()) == last)
            .or(paired.first())
            .map(|d| (*d).clone())
    }

    pub(super) fn poll_devices(&self, purpose: PollPurpose) {
        let platform = self.platform.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let devices = platform.paired_devices().await;
            let _ = tx.send(Msg::Devices { purpose, devices });
        });
    }

    /// Starts a connection attempt (after listing the devices).
    pub(super) fn connect(&mut self) {
        self.cancel(TimerKind::Retry);
        if self.link_handle.is_some() || self.link == LinkState::Connecting {
            let why = if self.link_handle.is_some() {
                "channel open"
            } else {
                "connecting"
            };
            self.info(&format!("connect skipped ({why})"));
            return;
        }
        self.poll_devices(PollPurpose::Connect);
    }

    /// Second half of `connect`, once the device list is in.
    pub(super) fn connect_with(&mut self, devices: Vec<BtDevice>) {
        self.devices = devices;
        // The list may have taken a moment; something else may have connected meanwhile.
        if self.link_handle.is_some() || self.link == LinkState::Connecting {
            return;
        }
        let Some(buds) = self.find_buds_in(&self.devices) else {
            self.set_link(LinkState::Searching);
            if let Some(name) = self.paired_buds(&self.devices).and_then(|d| d.name)
                && name != self.device_name
            {
                self.device_name = name;
            }
            self.page_buds();
            self.schedule_retry();
            return;
        };
        if let Some(previous) = &self.device
            && previous.address != buds.address
        {
            self.reset_device_state();
        }
        if let Some(name) = &buds.name {
            self.device_name = name.clone();
        }
        let address = buds.address_string();
        self.address = Some(address.clone());
        self.settings
            .update(|s| s.last_buds_address = Some(address.clone()));
        self.device = Some(buds.clone());
        self.set_link(LinkState::Connecting);
        self.info(&format!("connecting to {} ({address})", self.device_name));

        self.attempt += 1;
        self.next_link_id += 1;
        let (attempt, link_id) = (self.attempt, self.next_link_id);
        let sink = LinkSink {
            id: link_id,
            tx: self.tx.clone(),
        };
        let hint = self
            .settings
            .read(|s| s.connect_hints.get(&address).cloned());
        let platform = self.platform.clone();
        let tx = self.tx.clone();
        let log = self.log.log_fn();
        self.open_task = Some(tokio::spawn(async move {
            let result = platform.open(&buds, hint, sink, log).await;
            let _ = tx.send(Msg::LinkOpened {
                attempt,
                link_id,
                result,
            });
        }));
    }

    pub(super) fn link_opened(
        &mut self,
        attempt: u64,
        link_id: u64,
        result: Result<Box<dyn Link>, OpenError>,
    ) {
        // A result for an attempt we gave up on must not leak its socket.
        if attempt != self.attempt || self.link != LinkState::Connecting {
            if let Ok(link) = result {
                link.close();
            }
            return;
        }
        self.open_task = None;
        match result {
            Err(error) => {
                self.info(&format!("could not open the control channel: {error}"));
                self.set_link(LinkState::Searching);
                self.schedule_retry();
            }
            Ok(link) => {
                self.channel_id = link.channel();
                // Remembered only once the buds answer: a service that accepts the connection but
                // stays silent is not the control channel.
                self.pending_hint = link.connect_hint();
                self.link_handle = Some(link);
                self.current_link = link_id;
                if let Some(address) = self.address.clone() {
                    self.settings.update(|s| {
                        if !s.known_buds.contains(&address) {
                            s.known_buds.push(address);
                        }
                    });
                }
                self.did_open();
            }
        }
    }

    /// The channel is open: start talking to the buds.
    fn did_open(&mut self) {
        self.info(&format!(
            "connected{}",
            self.channel_id
                .map_or(String::new(), |c| format!(" on channel {c}"))
        ));
        self.set_link(LinkState::Connected);
        self.retry_count = 0;
        self.cancel(TimerKind::Retry);
        self.last_received = None;
        // A mode picked while disconnected is newer than the remembered one, so it wins.
        let remember = self.settings.read(|s| s.remember_settings);
        self.restore_noise = remember && self.queued_mode.is_none();
        self.restore_low_latency = remember;
        self.noise_fresh = false;
        self.switch_fresh = false;
        self.send(cmd::HELLO, vec![]);
        self.send(cmd::PRODUCT_ID, vec![]);
        // A mode requested while disconnected waits for the product ID, so it uses the right layout.
        self.schedule(TimerKind::ApplyQueuedMode, APPLY_QUEUED_MODE_DELAY);
        // Ask for pushes: battery, wear status and noise mode.
        self.send(cmd::SUBSCRIBE, encode::SUBSCRIBE.to_vec());
        self.send(cmd::WEAR_STATUS, vec![]);
        self.send(cmd::MULTIPOINT_QUERY, vec![]);
        self.send(cmd::EQ_QUERY, vec![]);
        self.send(cmd::EQ_LIST_QUERY, encode::EQ_LIST_QUERY.to_vec());
        self.send(cmd::SWITCH_QUERY, encode::LOW_LATENCY_QUERY.to_vec());
        self.send(cmd::GESTURE_QUERY, vec![]);
        self.refresh();
    }

    /// The remote side closed the channel.
    pub(super) fn link_closed(&mut self) {
        self.info("channel closed");
        self.close_channel();
        self.schedule_retry();
    }

    pub(super) fn close_channel(&mut self) {
        self.cancel(TimerKind::ReplyWatchdog);
        if let Some(task) = self.open_task.take() {
            task.abort();
        }
        self.attempt += 1; // any open still in flight is now stale
        if let Some(link) = self.link_handle.take() {
            link.close();
        }
        self.current_link = 0;
        self.channel_id = None;
        self.pending_hint = None;
        self.parser = FrameParser::new();
        self.outstanding_noise_queries.clear();
        self.stale_noise_queries.clear();
        if self.link != LinkState::Searching {
            self.set_link(LinkState::Searching);
        }
    }

    pub(super) fn set_link(&mut self, new: LinkState) {
        let old = std::mem::replace(&mut self.link, new);
        if new == LinkState::Connecting && old != LinkState::Connecting {
            let budget = self.platform.connect_budget();
            self.schedule(TimerKind::ConnectingWatchdog, budget);
        } else if new != LinkState::Connecting {
            self.cancel(TimerKind::ConnectingWatchdog);
        }
    }

    /// Quick retries first (the buds often need a second after connecting), then every 8 s.
    pub(super) fn schedule_retry(&mut self) {
        let delay = RETRY_DELAYS[self.retry_count.min(RETRY_DELAYS.len() - 1)];
        self.retry_count += 1;
        self.info(&format!("retrying in {}s", delay.as_secs()));
        self.schedule(TimerKind::Retry, delay);
    }

    /// Forget everything learned about the previous buds when different ones connect.
    fn reset_device_state(&mut self) {
        self.product_id = None;
        self.capabilities = None;
        self.mode = None;
        self.level = None;
        self.noise_bit = None;
        self.left = None;
        self.right = None;
        self.case_battery = None;
        self.game_mode = None;
        self.eq_id = None;
        self.custom_eqs = Vec::new();
        self.gestures.clear();
        self.hosts = Vec::new();
        self.wear.clear();
    }

    // Paging

    /// Out of the case, the buds often reconnect only to the phone. Ask them to connect to this
    /// computer too. At most one attempt every 15 s; a page that never reported back stops
    /// blocking after 30 s.
    fn page_buds(&mut self) {
        let now = Instant::now();
        if self.page_in_flight
            && self
                .last_page
                .is_some_and(|t| now.duration_since(t) > PAGE_INFLIGHT_EXPIRY)
        {
            self.page_in_flight = false;
        }
        let recently = self
            .last_page
            .is_some_and(|t| now.duration_since(t) <= PAGE_MIN_INTERVAL);
        if !self.settings.read(|s| s.auto_connect) || self.page_in_flight || recently {
            return;
        }
        let Some(target) = self.paired_buds(&self.devices) else {
            return;
        };
        self.page_in_flight = true;
        self.last_page = Some(now);
        let name = target.name.clone().unwrap_or_else(|| "buds".into());
        self.info(&format!(
            "asking {name} to connect to this {}",
            self.platform.host_label()
        ));
        let platform = self.platform.clone();
        let tx = self.tx.clone();
        let log = self.log.log_fn();
        tokio::spawn(async move {
            let result = platform.page(&target, log).await;
            let _ = tx.send(Msg::PageDone(result));
        });
    }

    pub(super) fn page_done(&mut self, result: Result<(), String>) {
        self.page_in_flight = false;
        if let Err(error) = result {
            self.info(&format!("buds not reachable ({error})"));
        }
        // On success the next device poll sees the buds connect and takes it from there.
    }

    // Presence (polling stands in for Bluetooth notifications)

    /// Every few seconds: notice a computer wake-up, then ask for the device list.
    pub(super) fn health_tick(&mut self) {
        let now = self.platform.wall_time();
        let gap = now.duration_since(self.last_wall).unwrap_or(Duration::ZERO);
        self.last_wall = now;
        if gap > RESUME_GAP {
            // After sleep the link may be dead without a close callback, or still fine. Check it:
            // a refresh that gets no reply reconnects, a working link is left alone.
            self.info("woke from sleep, checking the connection");
            self.schedule(TimerKind::Resume, RESUME_DELAY);
        }
        self.poll_devices(PollPurpose::Health);
    }

    /// Handles the periodic device list: buds appearing, disappearing, and the safety net.
    pub(super) fn health(&mut self, devices: Vec<BtDevice>) {
        self.devices = devices;
        let now = Instant::now();
        let connected: HashSet<[u8; 6]> = self
            .devices
            .iter()
            .filter(|d| d.connected && self.looks_like_buds(d))
            .map(|d| d.address)
            .collect();

        // Presence bookkeeping: when did each buds' connection to this computer start?
        let first_poll = self.prev_connected.is_none();
        self.connected_since
            .retain(|addr, _| connected.contains(addr));
        for addr in &connected {
            // Already connected at startup counts as "long ago".
            let since = if first_poll {
                now.checked_sub(CONNECT_AFTER_DEVICE_UP).unwrap_or(now)
            } else {
                now
            };
            self.connected_since.entry(*addr).or_insert(since);
        }

        // New connection: the buds came out of the case or back in range.
        if let Some(previous) = self.prev_connected.take() {
            let arrived: Vec<[u8; 6]> = connected.difference(&previous).copied().collect();
            for addr in arrived {
                if self.link_handle.is_none() {
                    let name = self
                        .devices
                        .iter()
                        .find(|d| d.address == addr)
                        .and_then(|d| d.name.clone())
                        .unwrap_or_else(|| "buds".into());
                    self.info(&format!(
                        "{name} connected to this {}",
                        self.platform.host_label()
                    ));
                    self.retry_count = 0;
                    // Give the buds a moment to finish setting up audio profiles before RFCOMM.
                    self.schedule(TimerKind::Retry, CONNECT_AFTER_DEVICE_UP);
                }
            }
        }
        self.prev_connected = Some(connected.clone());

        // The buds we're talking to went away. Only when the list still has
        // them, marked not connected: a paired device missing from the list altogether means the
        // listing itself failed (Windows returns nothing on any error), and dropping a working link
        // for that would be wrong. A link that really died closes itself or trips the reply watchdog.
        if self.link == LinkState::Connected
            && let Some(device) = &self.device
            && self
                .devices
                .iter()
                .any(|d| d.address == device.address && !d.connected)
        {
            let name = self.device_name.clone();
            self.info(&format!(
                "{name} disconnected from this {}",
                self.platform.host_label()
            ));
            self.close_channel();
            self.left = None;
            self.right = None;
            self.case_battery = None;
            self.retry_count = 0;
            self.schedule_retry();
            return;
        }

        // Safety net: buds are connected to this computer but we aren't talking to them. Wait for
        // the settling time first so this can't race the delayed connect above.
        if self.link == LinkState::Searching
            && let Some(buds) = self.find_buds_in(&self.devices)
            && self
                .connected_since
                .get(&buds.address)
                .is_some_and(|t| now.duration_since(*t) >= CONNECT_AFTER_DEVICE_UP)
        {
            self.info("buds are connected but the app isn't, connecting");
            self.retry_count = 0;
            self.connect();
        }
    }
}
