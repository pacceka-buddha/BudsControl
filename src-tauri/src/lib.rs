//! Buds Control tray app: a Tauri shell around `buds-core`.
//!
//! The controller (all the earbuds logic) runs as a task; this crate shows its state in a tray icon,
//! a flyout window and a settings window, and forwards the UI's commands to it.

mod app_windows;
mod commands;
mod positioning;
mod state;
mod theme;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use buds_core::controller::{Controller, ControllerHandle};
use buds_core::fake::FakePlatform;
use buds_core::packet_log::PacketLog;
use buds_core::platform::Platform;
use buds_core::settings::SettingsStore;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tokio::sync::broadcast::error::RecvError;

use crate::state::{AppState, FlyoutState};

/// Quits the app: closes the link to the buds first.
pub fn quit(app: &AppHandle) {
    app.state::<AppState>().handle.shutdown();
    app.exit(0);
}

fn spawn_controller<P: Platform>(
    platform: P,
    settings: SettingsStore,
    log: PacketLog,
) -> ControllerHandle {
    let (controller, handle) = Controller::new(Arc::new(platform), settings, log);
    tauri::async_runtime::spawn(controller.run());
    handle
}

/// `BUDS_FAKE=1` runs against simulated buds (no Bluetooth needed).
fn fake_requested() -> bool {
    std::env::var("BUDS_FAKE").is_ok_and(|v| !v.is_empty() && v != "0")
}

pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second launch tells the running app to show its flyout, then exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app_windows::show_flyout(app, None);
        }))
        // Launch at login: the entry starts the app with --background, so it stays quiet in the tray.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_settings,
            commands::update_settings,
            commands::set_mode,
            commands::cycle_mode,
            commands::set_noise_bit,
            commands::set_low_latency,
            commands::set_eq,
            commands::set_custom_eq,
            commands::find_buds,
            commands::set_gesture,
            commands::reconnect,
            commands::send_raw,
            commands::scan_queries,
            commands::get_log,
            commands::clear_log,
            commands::set_log_paused,
            commands::get_log_paused,
            commands::get_autostart,
            commands::set_autostart,
            commands::app_version,
            commands::export_log,
            commands::get_accent_color,
            commands::flyout_resize,
            commands::flyout_hide,
            commands::set_menu_open,
            commands::open_settings,
            commands::quit,
            commands::is_fake,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Buds Control");

    app.run(|_app, event| {
        // The tray app has no "main window": closing the settings window must not end the process.
        if let RunEvent::ExitRequested { api, code, .. } = event
            && code.is_none()
        {
            api.prevent_exit();
        }
    });
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let background = std::env::args().any(|a| a == "--background");
    let fake = fake_requested();

    let (settings, log) = if fake {
        (SettingsStore::in_memory(), PacketLog::new())
    } else {
        (
            buds_core::paths::settings_path()
                .map_or_else(SettingsStore::in_memory, SettingsStore::load),
            buds_core::paths::log_path().map_or_else(PacketLog::new, |p| PacketLog::with_file(&p)),
        )
    };

    let handle = if fake {
        let platform = FakePlatform::new();
        if std::env::var("BUDS_FAKE_SWEEP").is_ok_and(|v| !v.is_empty() && v != "0") {
            sweep_battery(platform.clone());
        }
        spawn_controller(platform, settings, log)
    } else {
        #[cfg(windows)]
        {
            spawn_controller(buds_bt::WindowsPlatform::new(), settings, log)
        }
        #[cfg(not(windows))]
        {
            // No native backend on this OS yet (Phase 11 / 12): use the simulated buds.
            spawn_controller(FakePlatform::new(), settings, log)
        }
    };
    handle.start();

    app.manage(AppState {
        handle: handle.clone(),
        flyout: Mutex::new(FlyoutState::default()),
        fake,
    });
    app_windows::create_flyout(app.handle())?;
    let tray_state = tray::build(app.handle())?;
    tray::update(app.handle(), &tray_state, &handle.snapshot());
    app.manage(tray_state);

    forward_events(app.handle().clone(), &handle);

    // A manual launch shows the flyout once so the user sees where the app lives; the autostart
    // entry passes --background and stays quiet in the tray.
    if !background {
        let app = app.handle().clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            app_windows::show_flyout(&app, None);
        });
    }
    Ok(())
}

/// Development aid (`BUDS_FAKE=1 BUDS_FAKE_SWEEP=1`): the simulated buds' battery sweeps up and down
/// ten times a second, so the tray icon is redrawn constantly (used to check it doesn't leak).
fn sweep_battery(platform: FakePlatform) {
    tauri::async_runtime::spawn(async move {
        let mut level: i32 = 100;
        let mut step: i32 = -1;
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            level += step;
            if !(1..=99).contains(&level) {
                step = -step;
            }
            let l = level as u8;
            platform.push_battery(
                (l, false),
                (l.saturating_add(1).min(100), false),
                (50, false),
            );
        }
    });
}

/// Pushes controller state, settings and packet-log batches to the web UI, and keeps the tray current.
fn forward_events(app: AppHandle, handle: &ControllerHandle) {
    // Controller state -> `buds://state` + tray.
    let mut state_rx = handle.watch();
    let state_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let snapshot = state_rx.borrow_and_update().clone();
            let _ = state_app.emit("buds://state", &snapshot);
            tray::update(&state_app, &state_app.state::<tray::TrayState>(), &snapshot);
            if state_rx.changed().await.is_err() {
                break;
            }
        }
    });

    // The taskbar colour and the display scale can change without the buds' state changing (the user
    // switches Windows to light mode, moves to a monitor with another scaling): look again regularly.
    let icon_app = app.clone();
    let icon_handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            tray::update(
                &icon_app,
                &icon_app.state::<tray::TrayState>(),
                &icon_handle.snapshot(),
            );
        }
    });

    // Settings -> `buds://settings`.
    let mut settings_rx = handle.settings().subscribe();
    let settings_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while settings_rx.changed().await.is_ok() {
            let settings = settings_rx.borrow_and_update().clone();
            let _ = settings_app.emit("buds://settings", &settings);
        }
    });

    // Packet log -> `buds://log`, batched every ~100 ms so a burst doesn't flood the webview.
    let mut log_rx = handle.log().subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            let first = match log_rx.recv().await {
                Ok(entry) => entry,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            };
            let mut batch = vec![first];
            let _ = tokio::time::timeout(Duration::from_millis(100), async {
                loop {
                    match log_rx.recv().await {
                        Ok(entry) => batch.push(entry),
                        Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => break,
                    }
                }
            })
            .await;
            let _ = app.emit("buds://log", &batch);
        }
    });
}
