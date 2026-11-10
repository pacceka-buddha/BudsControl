//! `budsctl`: command-line control of OnePlus / OPPO / realme earbuds.
//!
//! Runs its own controller against the real Bluetooth stack (or the simulated buds with `--fake`),
//! executes one command with `buds_core::cli::execute` and exits. The same commands will later be
//! forwarded to the running tray app (Phase 9).

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
";

#[derive(Parser)]
#[command(name = "budsctl", version, about = "Control OnePlus / OPPO / realme earbuds from the command line", after_help = COMMANDS)]
struct Args {
    /// Talk to the buds directly even if the tray app is running (the only mode for now).
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

#[cfg(windows)]
async fn native(args: &Args) -> i32 {
    let settings = buds_core::paths::settings_path()
        .map_or_else(SettingsStore::in_memory, SettingsStore::load);
    let log =
        buds_core::paths::log_path().map_or_else(PacketLog::new, |p| PacketLog::with_file(&p));
    with_platform(buds_bt::NativePlatform::new(), args, settings, log).await
}

#[cfg(not(windows))]
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
    let handle = spawn(platform, settings, log);
    let mut stdout = std::io::stdout();
    let code = cli::execute(&handle, &args.command, &mut stdout).await;
    let _ = stdout.flush();
    handle.shutdown();
    tokio::time::sleep(Duration::from_millis(150)).await; // let the link close
    code
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
