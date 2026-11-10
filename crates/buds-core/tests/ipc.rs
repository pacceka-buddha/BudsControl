//! `budsctl` <-> app: the IPC server runs the same commands as the command line against a live
//! controller. Real time (local sockets aren't driven by tokio's paused clock).

use std::sync::Arc;
use std::time::Duration;

use buds_core::controller::spawn;
use buds_core::fake::FakePlatform;
use buds_core::ipc::{forward_on, serve_on};
use buds_core::packet_log::PacketLog;
use buds_core::protocol::cmd;
use buds_core::settings::SettingsStore;
use buds_core::types::LinkState;

fn unique(tag: &str) -> String {
    format!("budscontrol-test-{}-{tag}", std::process::id())
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// A controller on the fake buds, already running (like the tray app), and its IPC server.
async fn running_app(name: &str) -> (buds_core::controller::ControllerHandle, FakePlatform) {
    let fake = FakePlatform::new();
    let handle = spawn(
        Arc::new(fake.clone()),
        SettingsStore::in_memory(),
        PacketLog::new(),
    );
    handle.start();
    let server = handle.clone();
    let name = name.to_string();
    tokio::spawn(async move {
        let _ = serve_on(&name, server).await;
    });
    // Wait until the app is connected and the server listens.
    let mut state = handle.watch();
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.borrow().link != LinkState::Connected {
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("the fake buds connect");
    tokio::time::sleep(Duration::from_millis(400)).await;
    (handle, fake)
}

#[tokio::test]
async fn status_through_the_socket_matches_a_direct_run() {
    let name = unique("status");
    let (_handle, _fake) = running_app(&name).await;

    let mut out = Vec::new();
    let code = forward_on(&name, &args(&["status"]), &mut out)
        .await
        .expect("the app is listening")
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(code, 0, "{text}");
    for line in [
        "device:  OnePlus Buds 4 [065414] connected",
        "battery: L 80%  R 82%  case 45%",
        "eq:      0 Balanced",
        "gesture: left doubleTap=playPause tripleTap=next hold=noiseControl",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "missing {line:?} in:\n{text}"
        );
    }
}

#[tokio::test]
async fn a_command_changes_the_running_buds() {
    let name = unique("anc");
    let (handle, fake) = running_app(&name).await;

    let mut out = Vec::new();
    let code = forward_on(&name, &args(&["--cli", "max"]), &mut out)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(code, 0);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.lines().any(|l| l == "mode:    anc (max) [bit 4]"),
        "{text}"
    );
    // The running controller itself changed (what the flyout shows), and the buds got one command.
    assert_eq!(
        handle.snapshot().mode,
        Some(buds_core::types::NoiseMode::Anc)
    );
    assert_eq!(fake.count_sent(cmd::SET_NOISE), 1);
    // A second `status` did not restart anything: still one hello.
    let mut out = Vec::new();
    forward_on(&name, &args(&["status"]), &mut out)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fake.count_sent(cmd::HELLO), 1);
}

#[tokio::test]
async fn exit_codes_and_usage_errors_come_through() {
    let name = unique("exit");
    let (_handle, _fake) = running_app(&name).await;

    let mut out = Vec::new();
    let code = forward_on(&name, &args(&["raw"]), &mut out)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(code, 2);
    assert!(String::from_utf8(out).unwrap().contains("usage"));
}

#[tokio::test]
async fn several_clients_at_once() {
    let name = unique("many");
    let (_handle, _fake) = running_app(&name).await;
    let a = args(&["status"]);
    let run = |n: String, a: Vec<String>| async move {
        let mut out = Vec::new();
        let code = forward_on(&n, &a, &mut out).await.unwrap().unwrap();
        (code, String::from_utf8(out).unwrap())
    };
    let (r1, r2, r3) = tokio::join!(
        run(name.clone(), a.clone()),
        run(name.clone(), a.clone()),
        run(name.clone(), a)
    );
    for (code, text) in [r1, r2, r3] {
        assert_eq!(code, 0);
        assert!(text.contains("device:  OnePlus Buds 4"));
    }
}

#[tokio::test]
async fn no_app_listening_means_none() {
    let mut out = Vec::new();
    let result = forward_on(&unique("nobody"), &args(&["status"]), &mut out).await;
    assert!(result.is_none());
    assert!(out.is_empty());
}
