//! Global keyboard shortcuts (work in any app, no special permission): which combination does what,
//! which of them another program already owns, and what happens when one is pressed.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use buds_core::types::{LinkState, NoiseMode};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tokio::sync::{mpsc, oneshot};

use crate::hud::{self, HudPayload};
use crate::state::AppState;

/// The actions a shortcut can trigger, as stored in the settings (`shortcuts` map keys).
pub const ACTIONS: [&str; 4] = ["cycle", "anc", "transparency", "off"];

/// How long to wait for the controller to apply the change before showing the HUD anyway.
const CONFIRM_TIMEOUT: Duration = Duration::from_millis(700);
/// The controller shows a new mode at once; the buds answer within ~100 ms and may refuse. Waiting
/// this long lets a refusal revert the state, so the HUD shows what the buds really did.
const SETTLE: Duration = Duration::from_millis(300);

/// A request to bring the registered shortcuts in line with the settings. The optional sender is told
/// when it has been done.
type Request = Option<oneshot::Sender<()>>;

pub struct HotkeyState {
    /// Actions whose combination could not be registered (another program owns it, or it is invalid).
    pub conflicts: Mutex<BTreeSet<String>>,
    /// The recorder is listening: no shortcut may fire, or pressing the old one would trigger it.
    pub recording: AtomicBool,
    tx: mpsc::UnboundedSender<Request>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<Request>>>,
}

impl HotkeyState {
    pub fn new() -> HotkeyState {
        let (tx, rx) = mpsc::unbounded_channel();
        HotkeyState {
            conflicts: Mutex::new(BTreeSet::new()),
            recording: AtomicBool::new(false),
            tx,
            rx: Mutex::new(Some(rx)),
        }
    }
}

impl Default for HotkeyState {
    fn default() -> Self {
        Self::new()
    }
}

/// Starts the worker that does all (re-)registration. The plugin's calls hop through the main thread,
/// so doing them from several threads at once (or from the main thread itself) can deadlock; one
/// worker, fed through a channel, never does.
pub fn start(app: &AppHandle) {
    let Some(mut rx) = app.state::<AppState>().hotkeys.rx.lock().unwrap().take() else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(first) = rx.recv().await {
            let mut waiters: Vec<oneshot::Sender<()>> = first.into_iter().collect();
            // Several requests in a row need only one pass: each pass reads the latest state.
            while let Ok(more) = rx.try_recv() {
                waiters.extend(more);
            }
            apply(&app);
            for waiter in waiters {
                let _ = waiter.send(());
            }
        }
    });
}

/// Whether the plugin can parse this accelerator (`Ctrl+Alt+KeyN`).
pub fn is_valid(accelerator: &str) -> bool {
    accelerator.parse::<Shortcut>().is_ok()
}

pub fn conflicts(app: &AppHandle) -> Vec<String> {
    app.state::<AppState>()
        .hotkeys
        .conflicts
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect()
}

/// Asks the worker to (re-)register every shortcut from the settings; returns at once.
pub fn register_all(app: &AppHandle) {
    let _ = app.state::<AppState>().hotkeys.tx.send(None);
}

/// Replaces what is registered with what the settings say. Only the worker calls this.
fn apply(app: &AppHandle) {
    let state = app.state::<AppState>();
    let global = app.global_shortcut();
    let _ = global.unregister_all();

    let mut failed = BTreeSet::new();
    if !state.hotkeys.recording.load(Ordering::SeqCst) {
        let shortcuts = state.handle.settings().get().shortcuts;
        for action in ACTIONS {
            let Some(accelerator) = shortcuts.get(action) else {
                continue;
            };
            let registered = accelerator
                .parse::<Shortcut>()
                .ok()
                .is_some_and(|shortcut| {
                    global
                        .on_shortcut(shortcut, move |app, _shortcut, event| {
                            if event.state == ShortcutState::Pressed {
                                perform(app, action);
                            }
                        })
                        .is_ok()
                });
            if !registered {
                failed.insert(action.to_string());
            }
        }
    }
    *state.hotkeys.conflicts.lock().unwrap() = failed.clone();
    let _ = app.emit("buds://conflicts", failed.into_iter().collect::<Vec<_>>());
}

/// While the recorder listens, nothing is registered; afterwards everything is registered again.
/// Returns once the change has been made.
pub async fn set_recording(app: &AppHandle, recording: bool) {
    let state = app.state::<AppState>();
    state.hotkeys.recording.store(recording, Ordering::SeqCst);
    let (done, wait) = oneshot::channel();
    if state.hotkeys.tx.send(Some(done)).is_ok() {
        let _ = wait.await;
    }
}

/// A shortcut was pressed: do it, then show what the mode is now.
fn perform(app: &AppHandle, action: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let handle = app.state::<AppState>().handle.clone();
        let mut rx = handle.watch();
        let before = rx.borrow().clone();
        let target = match action {
            "anc" => Some(NoiseMode::Anc),
            "transparency" => Some(NoiseMode::Transparency),
            "off" => Some(NoiseMode::Off),
            _ => None, // cycle: whatever comes next
        };
        match target {
            Some(mode) => handle.set_mode(mode, None),
            None => handle.cycle_mode(),
        }

        let connected = before.link == LinkState::Connected;
        let already = connected && target.is_some() && target == before.mode;
        if connected && !already {
            // Wait until the state shows the new mode (the controller applies it at once; this just
            // gives its message a moment to be processed).
            let _ = tokio::time::timeout(CONFIRM_TIMEOUT, async {
                while rx.changed().await.is_ok() {
                    let now = rx.borrow();
                    let changed = match target {
                        Some(mode) => now.mode == Some(mode),
                        None => now.mode != before.mode,
                    };
                    if changed || now.link != LinkState::Connected {
                        break;
                    }
                }
            })
            .await;
            tokio::time::sleep(SETTLE).await;
        }
        if handle.settings().get().show_hud {
            hud::show(&app, HudPayload::from_snapshot(&handle.snapshot()));
        }
    });
}
