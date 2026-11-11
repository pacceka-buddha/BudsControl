//! Phase 2 gate: the controller against the simulated Buds 4, with tokio's paused clock so retry
//! delays and watchdogs run instantly and deterministically.

use std::sync::Arc;
use std::time::Duration;

use buds_core::controller::{ControllerHandle, Snapshot, spawn};
use buds_core::fake::FakePlatform;
use buds_core::packet_log::PacketLog;
use buds_core::protocol::cmd;
use buds_core::settings::SettingsStore;
use buds_core::types::*;
use tokio::time::sleep;

struct Rig {
    fake: FakePlatform,
    handle: ControllerHandle,
    settings: SettingsStore,
    log: PacketLog,
}

/// A controller on the fake platform, not started yet.
fn rig() -> Rig {
    let fake = FakePlatform::new();
    let settings = SettingsStore::in_memory();
    let log = PacketLog::new();
    let handle = spawn(Arc::new(fake.clone()), settings.clone(), log.clone());
    Rig {
        fake,
        handle,
        settings,
        log,
    }
}

impl Rig {
    fn snap(&self) -> Snapshot {
        self.handle.snapshot()
    }

    /// How many log lines contain `needle`.
    fn log_count(&self, needle: &str) -> usize {
        self.log
            .entries()
            .iter()
            .filter(|e| e.text.contains(needle))
            .count()
    }
}

async fn secs(n: f64) {
    sleep(Duration::from_secs_f64(n)).await;
}

/// Payload of the most recent 0x0404 the fake buds received.
fn last_set_noise(fake: &FakePlatform) -> Vec<u8> {
    fake.sent()
        .into_iter()
        .rev()
        .find(|(c, _)| *c == cmd::SET_NOISE)
        .map(|(_, p)| p)
        .expect("a set-noise command was sent")
}

async fn connected_rig() -> Rig {
    let r = rig();
    r.handle.start();
    secs(1.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    r
}

// ---------------------------------------------------------------- connecting

#[tokio::test(start_paused = true)]
async fn connects_and_reads_the_buds() {
    let r = rig();
    r.handle.start();
    secs(1.0).await;
    let s = r.snap();
    assert_eq!(s.link, LinkState::Connected);
    assert_eq!(s.product_id.as_deref(), Some("065414"));
    assert_eq!(s.device_name, "OnePlus Buds 4");
    assert_eq!(s.model_name, "OnePlus Buds 4");
    assert_eq!(
        s.left,
        Some(Battery {
            level: 80,
            charging: false
        })
    );
    assert_eq!(s.right.map(|b| b.level), Some(82));
    assert_eq!(s.case.map(|b| b.level), Some(45));
    assert_eq!(s.buds_battery, Some(80));
    assert_eq!(s.mode, Some(NoiseMode::Off));
    assert_eq!(s.channel, Some(15));
    assert!(s.supports.noise_control && s.supports.equalizer && s.supports.low_latency);
    assert!(s.supports.custom_eq && s.supports.find_buds && s.supports.gestures);
    assert!(s.has_more_controls);
    assert_eq!(
        s.available_levels,
        [
            AncLevel::Smart,
            AncLevel::Max,
            AncLevel::Moderate,
            AncLevel::Mild,
            AncLevel::Adaptive
        ]
    );
    assert_eq!(s.hosts.len(), 2);
    assert!(s.hosts[1].is_this_host);
    assert_eq!(r.fake.count_sent(cmd::HELLO), 1);
    assert_eq!(
        r.settings.get().last_buds_address.as_deref(),
        Some("12:34:56:78:9A:BC")
    );
    assert!(
        r.settings
            .get()
            .known_buds
            .contains(&"12:34:56:78:9A:BC".to_string())
    );
    assert_eq!(
        r.settings
            .get()
            .connect_hints
            .get("12:34:56:78:9A:BC")
            .map(String::as_str),
        Some("channel 15")
    );
}

#[tokio::test(start_paused = true)]
async fn battery_pushes_update_the_state() {
    let r = connected_rig().await;
    r.fake.push_battery((64, true), (70, false), (0, false));
    secs(0.1).await;
    let s = r.snap();
    assert_eq!(
        s.left,
        Some(Battery {
            level: 64,
            charging: true
        })
    );
    assert_eq!(
        s.right,
        Some(Battery {
            level: 70,
            charging: false
        })
    );
    assert_eq!(s.case, None, "an empty, idle case is shown as unknown");
    assert_eq!(s.buds_battery, Some(64));
}

// ---------------------------------------------------------------- mode switching

#[tokio::test(start_paused = true)]
async fn anc_without_a_level_uses_last_then_first() {
    let r = connected_rig().await;

    // Nothing remembered: the first level the model has (Smart = bit 7).
    r.handle.set_mode(NoiseMode::Anc, None);
    secs(0.2).await;
    assert_eq!(last_set_noise(&r.fake), [0x01, 0x01, 0x80]);
    assert_eq!(r.snap().mode, Some(NoiseMode::Anc));
    assert_eq!(r.snap().level, Some(AncLevel::Smart));

    // Explicit level (Max = bit 4), then away and back: the last level wins.
    r.handle.set_mode(NoiseMode::Anc, Some(AncLevel::Max));
    secs(0.2).await;
    assert_eq!(last_set_noise(&r.fake), [0x01, 0x01, 0x10]);
    r.handle.set_mode(NoiseMode::Transparency, None);
    secs(1.0).await;
    assert_eq!(last_set_noise(&r.fake), [0x01, 0x01, 0x04]); // Transparency is set with bit 2
    assert_eq!(r.snap().mode, Some(NoiseMode::Transparency)); // ... and reported back as bit 8
    assert_eq!(r.snap().level, None);
    r.handle.set_mode(NoiseMode::Anc, None);
    secs(0.2).await;
    assert_eq!(last_set_noise(&r.fake), [0x01, 0x01, 0x10]);
    assert_eq!(r.snap().level, Some(AncLevel::Max));
    assert_eq!(r.settings.get().last_anc_level, Some(AncLevel::Max));
}

#[tokio::test(start_paused = true)]
async fn ui_updates_at_once_and_a_stale_reply_does_not_flip_it_back() {
    let r = connected_rig().await;
    assert_eq!(r.snap().mode, Some(NoiseMode::Off));

    // A noise query is answered late with the *old* mode while a change is already on its way.
    r.fake.delay_next_noise_reply(Duration::from_millis(300));
    r.handle.refresh();
    r.handle.set_mode(NoiseMode::Transparency, None);
    secs(0.05).await;
    assert_eq!(r.snap().mode, Some(NoiseMode::Transparency));
    secs(0.5).await; // the stale reply has arrived by now
    assert_eq!(r.snap().mode, Some(NoiseMode::Transparency));
    secs(2.0).await;
    assert_eq!(r.snap().mode, Some(NoiseMode::Transparency));
    assert_eq!(r.fake.reported_noise_bit(), 8);
}

#[tokio::test(start_paused = true)]
async fn cycle_goes_anc_transparency_off() {
    let r = connected_rig().await;
    // Off -> ANC (Smart) -> Transparency -> Off
    for expected in [NoiseMode::Anc, NoiseMode::Transparency, NoiseMode::Off] {
        r.handle.cycle_mode();
        secs(1.0).await;
        assert_eq!(r.snap().mode, Some(expected));
    }
}

#[tokio::test(start_paused = true)]
async fn mode_changed_on_the_buds_shows_up() {
    let r = connected_rig().await;
    r.fake.set_reported_noise_bit(4); // long press on the bud: ANC Max
    r.handle.refresh();
    secs(0.5).await;
    assert_eq!(r.snap().mode, Some(NoiseMode::Anc));
    assert_eq!(r.snap().level, Some(AncLevel::Max));
    assert_eq!(r.snap().noise_bit, Some(4));
}

#[tokio::test(start_paused = true)]
async fn mode_chosen_while_disconnected_is_sent_after_the_product_id() {
    let r = rig();
    r.fake.set_connected(false);
    r.handle.set_mode(NoiseMode::Anc, None);
    r.handle.start();
    secs(5.0).await;
    assert_eq!(r.snap().link, LinkState::Searching);
    assert_eq!(r.fake.count_sent(cmd::SET_NOISE), 0);

    r.fake.set_connected(true);
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    let sent = r.fake.sent();
    let product = sent
        .iter()
        .position(|(c, _)| *c == cmd::PRODUCT_ID)
        .unwrap();
    let set = sent.iter().position(|(c, _)| *c == cmd::SET_NOISE).unwrap();
    assert!(product < set, "the mode waits for the product ID");
    assert_eq!(r.fake.count_sent(cmd::SET_NOISE), 1);
    assert_eq!(last_set_noise(&r.fake), [0x01, 0x01, 0x80]);
}

// ---------------------------------------------------------------- other controls

#[tokio::test(start_paused = true)]
async fn low_latency_eq_custom_eq_and_gestures() {
    let r = connected_rig().await;
    assert_eq!(r.snap().low_latency, Some(false));

    r.handle.set_low_latency(true);
    secs(1.0).await;
    assert_eq!(r.snap().low_latency, Some(true));
    assert!(r.fake.low_latency());

    r.handle.set_eq(2);
    secs(0.5).await;
    assert_eq!(r.snap().eq_id, Some(2));
    let names: Vec<String> = r.snap().eq_choices.iter().map(|c| c.name.clone()).collect();
    assert_eq!(names, ["Balanced", "Vocal", "Bass", "Custom"]);

    // Custom EQ: select it, then change bands (gains are clamped to the buds' -6..+6).
    r.handle.set_eq(3);
    secs(0.5).await;
    assert!(r.snap().active_custom_eq.is_some());
    r.handle.set_custom_eq(vec![10, -10, 1, 2, 3, 4]);
    secs(0.5).await;
    let gains: Vec<i32> = r
        .snap()
        .active_custom_eq
        .unwrap()
        .bands
        .iter()
        .map(|b| b.gain)
        .collect();
    assert_eq!(gains, [6, -6, 1, 2, 3, 4]);
    assert_eq!(r.fake.count_sent(cmd::SET_CUSTOM_EQ), 1);

    // Gestures, with the actions this brand offers.
    let left = r.snap().gestures.left.clone().unwrap();
    let slot = left.double_tap.unwrap();
    assert_eq!(slot.current, GestureAction::PlayPause);
    let offered: Vec<GestureAction> = slot.options.iter().map(|o| o.action).collect();
    assert!(
        offered.contains(&GestureAction::NoiseControl)
            && offered.contains(&GestureAction::LowLatency)
    );
    assert!(!offered.contains(&GestureAction::AssistantRealme));
    r.handle
        .set_gesture(EarSide::Left, Gesture::DoubleTap, GestureAction::Next);
    secs(0.5).await;
    let now = r.snap().gestures.left.unwrap().double_tap.unwrap().current;
    assert_eq!(now, GestureAction::Next);
    assert_eq!(
        r.snap().gestures.right.unwrap().double_tap.unwrap().current,
        GestureAction::PlayPause
    );
}

#[tokio::test(start_paused = true)]
async fn find_buds_stops_by_itself_after_30_seconds() {
    let r = connected_rig().await;
    r.handle.find_buds(true);
    secs(1.0).await;
    assert!(r.snap().finding_buds);
    assert_eq!(r.fake.sent().last().unwrap(), &(cmd::FIND_BUDS, vec![1]));
    secs(29.0).await;
    assert!(r.snap().finding_buds, "still playing before 30 s");
    secs(1.5).await;
    assert!(!r.snap().finding_buds);
    assert_eq!(r.fake.sent().last().unwrap(), &(cmd::FIND_BUDS, vec![0]));
    assert_eq!(r.fake.count_sent(cmd::FIND_BUDS), 2);
}

// ---------------------------------------------------------------- connection robustness

#[tokio::test(start_paused = true)]
async fn retries_back_off_1_2_3_5_8_8_seconds() {
    let r = rig();
    r.fake.set_present(false);
    r.handle.start();
    secs(0.5).await;
    assert_eq!(r.log_count("retrying in"), 1);
    assert_eq!(r.log_count("retrying in 1s"), 1);
    secs(1.0).await; // t = 1.5: second attempt
    assert_eq!(r.log_count("retrying in 2s"), 1);
    secs(2.0).await; // t = 3.5
    assert_eq!(r.log_count("retrying in 3s"), 1);
    secs(3.0).await; // t = 6.5
    assert_eq!(r.log_count("retrying in 5s"), 1);
    secs(5.0).await; // t = 11.5
    assert_eq!(r.log_count("retrying in 8s"), 1);
    secs(8.0).await; // t = 19.5
    assert_eq!(r.log_count("retrying in 8s"), 2);
    assert_eq!(r.log_count("retrying in"), 6);
    assert_eq!(r.snap().link, LinkState::Searching);
}

/// Regression (found on real Linux hardware): a link that opens already dead made every request of the
/// post-connect burst fail, and each failure reconnected at once, ~25 open/close cycles in 300 ms.
/// A failed write must follow the retry timer like any other closed channel.
#[tokio::test(start_paused = true)]
async fn a_link_that_opens_dead_is_retried_on_the_timer_not_in_a_loop() {
    let r = rig();
    r.fake.open_dead(true);
    r.handle.start();
    secs(10.0).await;
    // One attempt per second at most (the first retry delay is 1 s), not a tight loop.
    let opens = r.fake.open_count();
    assert!(
        (2..=12).contains(&opens),
        "expected about one attempt per second, got {opens} in 10 s"
    );
    // Only the first failed write of a burst reports; the rest are dropped with the closed channel.
    assert!(r.log_count("write failed") <= opens);

    // Once the link is healthy again the connection settles.
    r.fake.open_dead(false);
    secs(15.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn silent_buds_trigger_a_reconnect_after_3_seconds() {
    let r = connected_rig().await;
    r.fake.drop_replies(true);
    r.handle.refresh();
    secs(2.5).await;
    assert_eq!(r.log_count("no reply from buds"), 0);
    secs(1.0).await;
    assert_eq!(r.log_count("no reply from buds, reconnecting"), 1);

    // Once the buds answer again the connection settles.
    r.fake.drop_replies(false);
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    assert_eq!(r.snap().product_id.as_deref(), Some("065414"));
    let seen = r.log_count("no reply from buds");
    secs(20.0).await;
    assert_eq!(
        r.log_count("no reply from buds"),
        seen,
        "no more reconnects once healthy"
    );
}

#[tokio::test(start_paused = true)]
async fn an_open_that_never_finishes_is_abandoned_after_the_budget() {
    let r = rig();
    r.fake.hang_open(true);
    r.handle.start();
    secs(1.0).await;
    assert_eq!(r.snap().link, LinkState::Connecting);
    secs(7.5).await; // t = 8.5: the 8 s watchdog fired (and the safety net already tried again)
    assert_eq!(r.log_count("stuck connecting, starting over"), 1);
    assert!(r.fake.open_count() >= 2, "a new attempt followed");

    // The attempt already hanging is only given up by its own watchdog; the next one succeeds.
    r.fake.hang_open(false);
    secs(12.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn a_failed_open_retries() {
    let r = rig();
    r.fake.fail_open(Some("host is down"));
    r.handle.start();
    secs(1.5).await;
    assert_eq!(r.snap().link, LinkState::Searching);
    assert!(r.log_count("could not open the control channel: host is down") >= 1);
    r.fake.fail_open(None);
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn buds_going_into_the_case_leave_no_stuck_connecting() {
    let r = connected_rig().await;
    r.fake.set_connected(false);
    secs(1.0).await;
    assert_eq!(r.snap().link, LinkState::Searching);
    secs(30.0).await;
    assert_eq!(r.snap().link, LinkState::Searching);
    // ... and out again: reconnects on its own.
    r.fake.set_connected(true);
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn a_failed_device_listing_does_not_drop_a_working_link() {
    let r = connected_rig().await;
    // The OS returns an empty list (an error on Windows) while the control link is fine.
    r.fake.set_present(false);
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    assert_eq!(r.log_count("disconnected from this"), 0);
}

#[tokio::test(start_paused = true)]
async fn buds_that_appear_are_connected_after_the_settling_time() {
    let r = rig();
    r.settings.update(|s| s.auto_connect = false);
    r.fake.set_connected(false);
    r.handle.start();
    // Run past the quick retries (1, 3, 6, 11 s) so only the presence check can connect.
    secs(12.5).await;
    r.fake.set_connected(true);
    let connected_at = r.fake.connected_at().unwrap();
    secs(10.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    let opened = *r.fake.open_times().last().unwrap();
    let gap = opened - connected_at;
    assert!(
        gap >= Duration::from_secs(3),
        "waited for the buds to settle, got {gap:?}"
    );
    assert!(
        gap <= Duration::from_millis(5500),
        "but not much longer, got {gap:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn paging_is_rate_limited_and_respects_auto_connect() {
    let r = rig();
    r.fake.phone_only();
    r.handle.start();
    secs(14.0).await;
    assert_eq!(r.fake.page_count(), 1);
    secs(6.0).await; // t = 20: the retry at 19 s is past the 15 s limit
    assert_eq!(r.fake.page_count(), 2);

    let off = rig();
    off.settings.update(|s| s.auto_connect = false);
    off.fake.phone_only();
    off.handle.start();
    secs(40.0).await;
    assert_eq!(off.fake.page_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_page_that_works_brings_the_buds_back() {
    let r = rig();
    r.fake.phone_only();
    r.fake.set_page_connects(true);
    r.handle.start();
    secs(15.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn waking_from_sleep_checks_the_link() {
    let r = connected_rig().await;
    r.fake.clear_sent();
    r.fake.skew_clock(Duration::from_secs(30));
    secs(5.0).await; // health tick notices the jump, 2 s later the link is refreshed
    assert_eq!(r.log_count("woke from sleep"), 1);
    assert!(
        r.fake.count_sent(cmd::BATTERY) >= 1,
        "refreshed after waking"
    );
    assert_eq!(r.snap().link, LinkState::Connected);
}

#[tokio::test(start_paused = true)]
async fn different_buds_start_from_a_clean_state() {
    let r = connected_rig().await;
    assert!(!r.snap().custom_eqs.is_empty());
    assert_eq!(r.snap().address.as_deref(), Some("12:34:56:78:9A:BC"));

    // OnePlus BulletsWireless Z2 ANC (050C14); its EQ list never arrives.
    r.fake.ignore_command(cmd::EQ_LIST_QUERY);
    r.fake.swap_device(
        [1, 2, 3, 4, 5, 6],
        "OnePlus BulletsWireless Z2 ANC",
        [0x14, 0x0C, 0x05],
        1,
    );
    secs(8.0).await;
    let s = r.snap();
    assert_eq!(s.link, LinkState::Connected);
    assert_eq!(s.address.as_deref(), Some("01:02:03:04:05:06"));
    assert_eq!(s.product_id.as_deref(), Some("050C14"));
    assert_eq!(s.device_name, "OnePlus BulletsWireless Z2 ANC");
    assert!(
        s.custom_eqs.is_empty(),
        "nothing left over from the previous buds"
    );
    assert!(s.available_levels.is_empty());
}

// ---------------------------------------------------------------- remembered settings

#[tokio::test(start_paused = true)]
async fn remembered_settings_are_restored_only_when_they_differ() {
    let r = rig();
    r.settings.update(|s| s.remember_settings = true);
    r.handle.start();
    secs(1.0).await;
    r.handle.set_mode(NoiseMode::Anc, Some(AncLevel::Max));
    secs(1.0).await;
    assert_eq!(r.fake.reported_noise_bit(), 4);

    // The buds go back in the case: they come out Off.
    r.fake.set_reported_noise_bit(3);
    r.fake.close_link();
    secs(6.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    assert_eq!(
        r.fake.count_sent(cmd::SET_NOISE),
        2,
        "the pick, then one restore"
    );
    assert_eq!(r.fake.reported_noise_bit(), 4);
    assert_eq!(r.snap().level, Some(AncLevel::Max));
    assert_eq!(r.log_count("restoring anc (max)"), 1);

    // A link that comes back as it was sends nothing.
    r.fake.clear_sent();
    r.fake.close_link();
    secs(6.0).await;
    assert_eq!(r.snap().link, LinkState::Connected);
    assert_eq!(r.fake.count_sent(cmd::SET_NOISE), 0);
}

#[tokio::test(start_paused = true)]
async fn nothing_is_restored_when_the_setting_is_off() {
    let r = rig();
    r.handle.start();
    secs(1.0).await;
    r.handle.set_mode(NoiseMode::Anc, Some(AncLevel::Max));
    secs(1.0).await;
    r.fake.set_reported_noise_bit(3);
    r.fake.close_link();
    secs(6.0).await;
    assert_eq!(r.fake.count_sent(cmd::SET_NOISE), 1);
    assert_eq!(r.snap().mode, Some(NoiseMode::Off));
}

#[tokio::test(start_paused = true)]
async fn remembered_low_latency_is_recorded_per_buds() {
    let r = rig();
    r.settings.update(|s| s.remember_settings = true);
    r.handle.start();
    secs(1.0).await;
    r.handle.set_low_latency(true);
    secs(1.0).await;
    let remembered = r.settings.get().remembered;
    assert_eq!(remembered["12:34:56:78:9A:BC"].low_latency, Some(true));
}

// ---------------------------------------------------------------- shutdown and log

#[tokio::test(start_paused = true)]
async fn shutdown_closes_the_link() {
    let r = connected_rig().await;
    r.handle.shutdown();
    secs(1.0).await;
    assert_eq!(r.snap().link, LinkState::Searching);
}

#[tokio::test(start_paused = true)]
async fn every_frame_is_logged() {
    let r = connected_rig().await;
    let text = r.log.export_text();
    assert!(
        text.contains("→ AA 07 00 00 00 01 01 00 00"),
        "hello request logged:\n{text}"
    );
    assert!(text.contains("←"), "replies logged");
}

// ---------------------------------------------------------------- CLI

async fn cli(r: &Rig, args: &[&str]) -> (i32, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut out: Vec<u8> = Vec::new();
    let code = buds_core::cli::execute(&r.handle, &args, &mut out).await;
    (code, String::from_utf8_lossy(&out).into_owned())
}

fn has_line(out: &str, line: &str) -> bool {
    out.lines().any(|l| l == line)
}

#[tokio::test(start_paused = true)]
async fn cli_status_prints_the_upstream_block() {
    let r = rig();
    let (code, out) = cli(&r, &["status"]).await;
    assert_eq!(code, 0, "{out}");
    for line in [
        "device:  OnePlus Buds 4 [065414] connected",
        "mode:    off [bit 3]",
        "battery: L 80%  R 82%  case 45%",
        "eq:      0 Balanced",
        "lowlat:  off",
        "model:   OnePlus Buds 4, capability bits: 6",
        "modes:   [\"off\", \"transparency\", \"anc\"] levels: [\"smart\", \"max\", \"moderate\", \"mild\", \"adaptive\"]",
        "shows:   noiseControl equalizer customEQ lowLatency findBuds gestures",
        "gesture: left doubleTap=playPause tripleTap=next hold=noiseControl",
        "gesture: right doubleTap=playPause tripleTap=next hold=noiseControl",
    ] {
        assert!(has_line(&out, line), "missing line {line:?} in:\n{out}");
    }
    // Packets are echoed with a millisecond stamp.
    assert!(out.contains("ms] -> AA"), "{out}");
    assert!(out.contains("ms] <- AA"), "{out}");
}

#[tokio::test(start_paused = true)]
async fn cli_actions_change_the_buds() {
    let r = rig();
    let (code, out) = cli(&r, &["max"]).await;
    assert_eq!(code, 0);
    assert!(has_line(&out, "mode:    anc (max) [bit 4]"), "{out}");

    let (_, out) = cli(&r, &["--cli", "lowlatency=on"]).await;
    assert!(has_line(&out, "lowlat:  on"), "{out}");

    let (_, out) = cli(&r, &["gesture=left,doubleTap,next"]).await;
    assert!(
        has_line(
            &out,
            "gesture: left doubleTap=next tripleTap=next hold=noiseControl"
        ),
        "{out}"
    );

    let (_, out) = cli(&r, &["eq=3"]).await;
    assert!(
        out.lines()
            .any(|l| l.starts_with("eq:      3 Custom 62Hz:0")),
        "{out}"
    );

    let (_, out) = cli(&r, &["custom=1,2,3,4,5,6"]).await;
    assert!(
        has_line(
            &out,
            "eq:      3 Custom 62Hz:1 250Hz:2 1000Hz:3 4000Hz:4 8000Hz:5 16000Hz:6"
        ),
        "{out}"
    );

    let (_, out) = cli(&r, &["transparency"]).await;
    assert!(has_line(&out, "mode:    transparency [bit 8]"), "{out}");
    let (_, out) = cli(&r, &["cycle"]).await;
    assert!(has_line(&out, "mode:    off [bit 3]"), "{out}");

    let (_, out) = cli(&r, &["bogus=1"]).await;
    assert!(out.contains("unknown action bogus"), "{out}");
}

#[tokio::test(start_paused = true)]
async fn cli_reports_not_connected_with_exit_code_1() {
    let r = rig();
    r.fake.set_connected(false);
    let (code, out) = cli(&r, &["status"]).await;
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("[?] searching"), "{out}");
}

#[tokio::test(start_paused = true)]
async fn cli_raw_scan_and_usage() {
    let r = rig();
    let (code, out) = cli(&r, &["raw", "0106"]).await;
    assert_eq!(code, 0);
    assert!(
        out.contains("-> AA 07 00 00 06 01"),
        "request echoed:\n{out}"
    );
    assert!(out.contains("<- AA"), "reply echoed:\n{out}");
    assert!(r.fake.count_sent(0x0106) >= 2);

    let (code, out) = cli(&r, &["raw"]).await;
    assert_eq!(code, 2);
    assert!(out.contains("usage"), "{out}");
    let (code, _) = cli(&r, &["raw", "zz"]).await;
    assert_eq!(code, 2);

    let r2 = rig();
    let (code, out) = cli(&r2, &["--scan", "buds4"]).await;
    assert_eq!(code, 0);
    assert!(
        out.lines()
            .any(|l| l.starts_with("buds4 0x8103 00 14 54 06")),
        "{out}"
    );
    assert!(
        out.lines().any(|l| l.starts_with("buds4 0x8106 00 03")),
        "{out}"
    );
}

#[tokio::test(start_paused = true)]
async fn cli_watch_prints_host_lists() {
    let r = rig();
    let (code, out) = cli(&r, &["--watch", "3"]).await;
    assert_eq!(code, 0);
    assert!(
        out.contains("HOSTS Phone connected=true self=false | This PC connected=true self=true"),
        "{out}"
    );
}
