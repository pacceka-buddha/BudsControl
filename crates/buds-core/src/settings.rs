//! Persistent user settings (the Swift app's UserDefaults), stored as one JSON file.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use ts_rs::TS;

use crate::types::{AncLevel, EarSide, NoiseMode};

/// The noise mode and low latency last picked in the app, per pair of buds. Only picks made here
/// count: the buds report Off on their way into the case, which mustn't be what comes back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Remembered {
    pub mode: Option<NoiseMode>,
    pub level: Option<AncLevel>,
    pub low_latency: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    /// Bring the buds back to this computer when they only joined the phone.
    pub auto_connect: bool,
    /// Put back the remembered noise mode and low latency when the buds connect.
    pub remember_settings: bool,
    /// Show the on-screen HUD when a shortcut changes the mode.
    pub show_hud: bool,
    pub developer_mode: bool,
    /// The "More controls" panel in the flyout is open.
    pub more_expanded: bool,
    pub gesture_side: EarSide,
    pub last_anc_level: Option<AncLevel>,
    /// Address of the buds we talked to last (`AA:BB:CC:DD:EE:FF`).
    pub last_buds_address: Option<String>,
    /// Addresses we have successfully talked to; recognises renamed buds.
    pub known_buds: Vec<String>,
    /// Address -> the connection candidate that worked last time.
    pub connect_hints: BTreeMap<String, String>,
    /// Action (`cycle`, `anc`, `transparency`, `off`) -> accelerator such as `Ctrl+Alt+N`.
    pub shortcuts: BTreeMap<String, String>,
    /// Address -> remembered noise mode / low latency.
    pub remembered: BTreeMap<String, Remembered>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_connect: true,
            remember_settings: false,
            show_hud: true,
            developer_mode: false,
            more_expanded: false,
            gesture_side: EarSide::Left,
            last_anc_level: None,
            last_buds_address: None,
            known_buds: Vec::new(),
            connect_hints: BTreeMap::new(),
            shortcuts: BTreeMap::new(),
            remembered: BTreeMap::new(),
        }
    }
}

/// Shared, observable settings. Clones share the same state. Changes are written to disk right
/// away (atomically) when the store has a path.
#[derive(Clone)]
pub struct SettingsStore {
    inner: Arc<Inner>,
}

struct Inner {
    value: Mutex<Settings>,
    path: Option<PathBuf>,
    tx: watch::Sender<Settings>,
}

impl SettingsStore {
    /// Settings that live only in memory (tests, `--fake`).
    pub fn in_memory() -> SettingsStore {
        Self::build(Settings::default(), None)
    }

    /// Loads `path`, falling back to defaults when it is missing or unreadable.
    pub fn load(path: PathBuf) -> SettingsStore {
        let value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self::build(value, Some(path))
    }

    fn build(value: Settings, path: Option<PathBuf>) -> SettingsStore {
        let (tx, _) = watch::channel(value.clone());
        SettingsStore {
            inner: Arc::new(Inner {
                value: Mutex::new(value),
                path,
                tx,
            }),
        }
    }

    pub fn get(&self) -> Settings {
        self.inner.value.lock().unwrap().clone()
    }

    /// Reads a value without cloning everything.
    pub fn read<R>(&self, f: impl FnOnce(&Settings) -> R) -> R {
        f(&self.inner.value.lock().unwrap())
    }

    /// Applies `f`; if anything changed it is saved and subscribers are notified.
    pub fn update(&self, f: impl FnOnce(&mut Settings)) {
        let snapshot = {
            let mut guard = self.inner.value.lock().unwrap();
            let before = guard.clone();
            f(&mut guard);
            if *guard == before {
                return;
            }
            guard.clone()
        };
        self.save(&snapshot);
        self.inner.tx.send_replace(snapshot);
    }

    pub fn subscribe(&self) -> watch::Receiver<Settings> {
        self.inner.tx.subscribe()
    }

    /// Write to `<file>.tmp`, then rename over the real file.
    fn save(&self, settings: &Settings) {
        let Some(path) = &self.inner.path else { return };
        let Ok(text) = serde_json::to_string_pretty(settings) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}
