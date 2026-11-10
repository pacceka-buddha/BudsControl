//! `budsctl`: command-line control of OnePlus / OPPO / realme earbuds.
//!
//! Runs its own controller against the real Bluetooth stack (or the simulated buds with `--fake`),
//! executes one command with `buds_core::cli::execute` and exits. When the tray app is running (and
//! neither `--direct` nor `--fake` is given) the command is forwarded to it over IPC instead, since the
//! buds allow only one control connection.

use std::io::Write;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use buds_core::cli;
use buds_core::controller::spawn;
use buds_core::packet_log::PacketLog;
use buds_core::platform::{BtDevice, Platform, name_looks_like_buds};
use buds_core::settings::SettingsStore;
use clap::Parser;

const COMMANDS: &str = "\
Commands:
  status                  show the buds' state (default)
  anc | transparency | off | smart | max | moderate | mild | adaptive
                          switch the noise mode (a level implies ANC)
  cycle                   next mode: ANC -> Transparency -> Off
  eq=<id>                 select an EQ preset (see `status`)
  custom=<g1,g2,...>      set the custom EQ band gains in dB
  lowlatency=on|off
  find=on|off             play / stop the loud find-my-earbuds sound
  gesture=<left|right>,<doubleTap|tripleTap|hold>,<action>
                          actions: none playPause assistant previous next
                          noiseControl volumeUp volumeDown lowLatency
  watch [seconds]         log everything the buds send (default 60 s)
  scan [label]            send every query once and print the last reply to each
  raw CMD:PAYLOAD ...     send raw commands (hex), e.g. raw 0106 010C:0101
  devices                 list the paired Bluetooth devices
  sdp                     show which control services the buds advertise, and their RFCOMM channels
  audio [reconnect]       list the buds' Windows audio endpoints; `reconnect` asks the audio driver
                          to (re)connect them
";

#[derive(Parser)]
#[command(name = "budsctl", version, about = "Control OnePlus / OPPO / realme earbuds from the command line", after_help = COMMANDS)]
struct Args {
    /// Talk to the buds directly even if the tray app is running (otherwise a running app is asked to
    /// do it, since the buds allow only one control connection).
    #[arg(long)]
    direct: bool,
    /// Use simulated OnePlus Buds 4 instead of real Bluetooth.
    #[arg(long)]
    fake: bool,
    /// The command to run (see below); also accepts the original `--cli anc` / `--watch 30` forms.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "COMMAND"
    )]
    command: Vec<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    // macOS: IOBluetooth delivers its callbacks on the main thread's run loop, so the main thread runs
    // that loop and the command (controller, runtime) runs on a worker thread.
    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        let done = Arc::new(AtomicBool::new(false));
        let finished = done.clone();
        let worker = std::thread::spawn(move || {
            let code = run_command(args);
            finished.store(true, Ordering::Relaxed);
            code
        });
        buds_bt::run_main_loop_until(&done);
        worker.join().unwrap_or(ExitCode::from(1))
    }
    #[cfg(not(target_os = "macos"))]
    run_command(args)
}

fn run_command(args: Args) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            eprintln!("budsctl: cannot start the async runtime: {error}");
            return ExitCode::from(3);
        }
    };
    let code = runtime.block_on(run(args));
    // Blocking Bluetooth threads may still be winding down; don't wait for them.
    runtime.shutdown_background();
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

async fn run(args: Args) -> i32 {
    // Commands that need the buds' control connection go to the running tray app, if there is one.
    let local_only = matches!(
        args.command.first().map(String::as_str),
        Some("devices" | "sdp" | "audio")
    );
    if !args.fake && !args.direct && !local_only {
        let mut stdout = std::io::stdout();
        match buds_core::ipc::forward(&args.command, &mut stdout).await {
            Some(Ok(code)) => return code,
            Some(Err(error)) => {
                eprintln!("budsctl: the running app stopped answering: {error}");
                return 1;
            }
            None => {} // no app running: talk to the buds ourselves
        }
    }
    if args.fake {
        return with_platform(
            buds_core::fake::FakePlatform::new(),
            &args,
            SettingsStore::in_memory(),
            PacketLog::new(),
        )
        .await;
    }
    native(&args).await
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
async fn native(args: &Args) -> i32 {
    let settings = buds_core::paths::settings_path()
        .map_or_else(SettingsStore::in_memory, SettingsStore::load);
    let log =
        buds_core::paths::log_path().map_or_else(PacketLog::new, |p| PacketLog::with_file(&p));
    with_platform(buds_bt::NativePlatform::new(), args, settings, log).await
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
async fn native(_args: &Args) -> i32 {
    eprintln!(
        "budsctl: there is no Bluetooth backend for this operating system yet (use --fake to try the commands)"
    );
    3
}

async fn with_platform<P: Platform>(
    platform: P,
    args: &Args,
    settings: SettingsStore,
    log: PacketLog,
) -> i32 {
    let platform = Arc::new(platform);
    if args.command.first().map(String::as_str) == Some("devices") {
        return list_devices(&platform.paired_devices().await);
    }
    if args.command.first().map(String::as_str) == Some("sdp") {
        return show_services(&platform.paired_devices().await).await;
    }
    if args.command.first().map(String::as_str) == Some("audio") {
        let reconnect = args.command.get(1).map(String::as_str) == Some("reconnect");
        return show_audio(&platform.paired_devices().await, reconnect);
    }
    let handle = spawn(platform, settings, log);
    let mut stdout = std::io::stdout();
    let code = cli::execute(&handle, &args.command, &mut stdout).await;
    let _ = stdout.flush();
    handle.shutdown();
    tokio::time::sleep(Duration::from_millis(150)).await; // let the link close
    code
}

/// `budsctl sdp`: the control services of the paired buds (a connected one is preferred).
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
async fn show_services(devices: &[BtDevice]) -> i32 {
    let buds = devices
        .iter()
        .filter(|d| name_looks_like_buds(d.name.as_deref().unwrap_or("")))
        .max_by_key(|d| d.connected);
    let Some(device) = buds else {
        eprintln!("budsctl: no paired earbuds found (see `budsctl devices`)");
        return 1;
    };
    println!(
        "{} {}",
        device.address_string(),
        device.name.as_deref().unwrap_or("(unnamed)")
    );
    #[cfg(any(windows, target_os = "macos"))]
    let lines = buds_bt::sdp_report(device.address);
    #[cfg(target_os = "linux")]
    let lines = buds_bt::sdp_report(device.address).await;
    for line in lines {
        println!("  {line}");
    }
    0
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
async fn show_services(_devices: &[BtDevice]) -> i32 {
    eprintln!("budsctl: service discovery is not available on this platform yet");
    3
}

/// `budsctl audio [reconnect]`: the buds' audio endpoints; optionally ask the driver to reconnect them.
#[cfg(windows)]
fn show_audio(devices: &[BtDevice], reconnect: bool) -> i32 {
    let Some(device) = devices
        .iter()
        .filter(|d| name_looks_like_buds(d.name.as_deref().unwrap_or("")))
        .max_by_key(|d| d.connected)
    else {
        eprintln!("budsctl: no paired earbuds found (see `budsctl devices`)");
        return 1;
    };
    for line in buds_bt::audio_report(device.address) {
        println!("{line}");
    }
    if reconnect {
        match buds_bt::audio_reconnect(device.address) {
            Ok(count) => println!("reconnect requested ({count} audio device)"),
            Err(reason) => {
                eprintln!("budsctl: reconnect failed: {reason}");
                return 1;
            }
        }
    }
    0
}

#[cfg(not(windows))]
fn show_audio(_devices: &[BtDevice], _reconnect: bool) -> i32 {
    eprintln!(
        "budsctl: `audio` is a Windows diagnostic (BlueZ and macOS connect the audio profiles themselves)"
    );
    3
}

fn list_devices(devices: &[BtDevice]) -> i32 {
    if devices.is_empty() {
        println!("no paired Bluetooth devices found (is Bluetooth on?)");
        return 1;
    }
    println!("{:<19} {:<10} {:<5} name", "address", "state", "buds");
    for d in devices {
        let name = d.name.as_deref().unwrap_or("(unnamed)");
        println!(
            "{:<19} {:<10} {:<5} {name}",
            d.address_string(),
            if d.connected { "connected" } else { "paired" },
            if name_looks_like_buds(name) {
                "yes"
            } else {
                ""
            },
        );
    }
    0
}
