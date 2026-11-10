//! Settings persistence and the packet log.

use std::path::PathBuf;

use buds_core::packet_log::{LOG_FILE_MAX, LOG_RING, PacketLog};
use buds_core::platform::LogDirection;
use buds_core::settings::{Settings, SettingsStore};
use buds_core::types::{AncLevel, EarSide, NoiseMode};

/// A fresh scratch directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("budscontrol-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn settings_defaults() {
    let s = Settings::default();
    assert!(s.auto_connect && s.show_hud);
    assert!(!s.remember_settings && !s.developer_mode && !s.more_expanded);
    assert_eq!(s.gesture_side, EarSide::Left);
    assert!(s.remembered.is_empty() && s.shortcuts.is_empty());
}

#[test]
fn settings_are_saved_and_loaded() {
    let dir = scratch("settings");
    let path = dir.join("nested").join("settings.json");
    let store = SettingsStore::load(path.clone());
    assert_eq!(store.get(), Settings::default()); // missing file: defaults

    store.update(|s| {
        s.remember_settings = true;
        s.last_anc_level = Some(AncLevel::Mild);
        s.shortcuts.insert("cycle".into(), "Ctrl+Alt+N".into());
        s.remembered
            .entry("AA:BB:CC:DD:EE:FF".into())
            .or_default()
            .mode = Some(NoiseMode::Anc);
    });
    assert!(path.exists());
    assert!(
        !path.with_extension("json.tmp").exists(),
        "temp file is renamed away"
    );

    let reloaded = SettingsStore::load(path.clone()).get();
    assert!(reloaded.remember_settings);
    assert_eq!(reloaded.last_anc_level, Some(AncLevel::Mild));
    assert_eq!(reloaded.shortcuts["cycle"], "Ctrl+Alt+N");
    assert_eq!(
        reloaded.remembered["AA:BB:CC:DD:EE:FF"].mode,
        Some(NoiseMode::Anc)
    );
    assert!(reloaded.auto_connect, "untouched defaults survive");

    // The JSON uses camelCase keys, and unknown / missing keys are tolerated.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"rememberSettings\": true"), "{text}");
    std::fs::write(&path, r#"{ "showHud": false, "somethingNew": 1 }"#).unwrap();
    let partial = SettingsStore::load(path.clone()).get();
    assert!(!partial.show_hud);
    assert!(partial.auto_connect);

    // A corrupt file falls back to defaults instead of failing.
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(SettingsStore::load(path).get(), Settings::default());
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn settings_notify_subscribers_only_on_change() {
    let store = SettingsStore::in_memory();
    let mut rx = store.subscribe();
    store.update(|s| s.show_hud = true); // already true: no change
    assert!(!rx.has_changed().unwrap());
    store.update(|s| s.show_hud = false);
    assert!(rx.has_changed().unwrap());
    assert!(!rx.borrow_and_update().show_hud);
}

#[test]
fn packet_log_keeps_the_last_600_entries() {
    let log = PacketLog::new();
    for n in 0..(LOG_RING + 50) {
        log.append(LogDirection::Info, &format!("line {n}"));
    }
    let entries = log.entries();
    assert_eq!(entries.len(), LOG_RING);
    assert_eq!(entries[0].text, "line 50");
    assert_eq!(
        entries.last().unwrap().text,
        format!("line {}", LOG_RING + 49)
    );
    log.clear();
    assert!(log.entries().is_empty());
}

#[test]
fn pause_freezes_the_list_but_not_the_file() {
    let dir = scratch("log-pause");
    let path = dir.join("BudsControl.log");
    let log = PacketLog::with_file(&path);
    log.append(LogDirection::Sent, "AA 07");
    log.set_paused(true);
    log.append(LogDirection::Received, "AA 08");
    log.set_paused(false);
    assert_eq!(log.entries().len(), 1);
    let file = std::fs::read_to_string(&path).unwrap();
    assert!(
        file.contains("→ AA 07") && file.contains("← AA 08"),
        "{file}"
    );
    assert!(log.export_text().contains("→ AA 07"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_oversized_log_file_is_started_fresh() {
    let dir = scratch("log-size");
    let path = dir.join("BudsControl.log");
    std::fs::write(&path, vec![b'x'; LOG_FILE_MAX as usize + 1]).unwrap();
    let log = PacketLog::with_file(&path);
    log.append(LogDirection::Info, "hello");
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size < 200, "file was truncated, size is {size}");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn subscribers_see_new_entries() {
    let log = PacketLog::new();
    let mut rx = log.subscribe();
    log.log_fn()(LogDirection::Sent, "AA 01");
    let entry = rx.recv().await.unwrap();
    assert_eq!(entry.direction, LogDirection::Sent);
    assert_eq!(entry.text, "AA 01");
}
