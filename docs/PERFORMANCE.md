# Performance and size

Measured figures for Buds Control 0.1.0: package sizes, memory, CPU, start-up and connection times, and how big the
project is. Everything here was measured on 2026-11-18 with release builds, except where a date is given. The last code
change in these builds is commit `be6412d`. If a number is missing, it was not measured.

## Test machines

| | Windows | Linux | macOS |
|---|---|---|---|
| CPU | AMD Ryzen 5 4600H, 6 cores / 12 threads | Intel Core i5-1235U, 12 threads | Apple silicon (MacBook Air) |
| RAM | 15.4 GB | 15 GB | not recorded |
| OS | Windows 11 Pro, build 26300 | Ubuntu 24.04.5, kernel 6.8, GNOME on X11 | macOS 27.0.1 |
| Earbuds | OnePlus Buds 4 | OnePlus Buds 4 | OnePlus Buds 3 |
| Runtime figures | simulated buds (`BUDS_FAKE=1`)¹ | real buds | not measured |

¹ The Buds 4 were connected to the Linux laptop at the time. Memory, CPU and start-up do not depend on the buds; the
connection times below come from real runs on each system.

## Package sizes

| Package | Size | Notes |
|---|---|---|
| Windows installer (`-setup.exe`) | 1,912,019 bytes (1.82 MiB) | Per-user install, no admin rights. WebView2 is not bundled; the installer downloads it only if it is missing. |
| Windows portable zip | 2,615,091 bytes (2.49 MiB) | `BudsControl.exe`, `budsctl.exe`, README, LICENSE, third-party notices. |
| `BudsControl.exe` | 4,653,056 bytes (4.44 MiB) | The tray app, with the web UI embedded. |
| `budsctl.exe` | 963,584 bytes (941 KiB) | The command-line tool. |
| Linux `.deb` | 4,172,036 bytes (3.98 MiB) | 9,448 kB once installed (`dpkg` Installed-Size). Uses the system WebKitGTK, BlueZ and AppIndicator packages. |
| Linux AppImage | 92,355,064 bytes (88.08 MiB) | Carries its own GTK and WebKitGTK libraries, which is why it is much larger than the `.deb`. |
| `BudsControl` / `budsctl` (Linux, in the `.deb`) | 7,575,256 / 1,991,480 bytes | 7.22 MiB / 1.90 MiB. |
| macOS `.dmg` (arm64) | 3,335,424 bytes (3.18 MiB) | Ad hoc signed, not notarised. |
| macOS `.app` | 5.67 MiB | As reported by the bundler. |

The web UI (flyout, settings, popup) is 129,499 bytes (126 KiB) of HTML, JavaScript and CSS, embedded in the binary.
Release builds use `opt-level = "s"`, link-time optimisation, one codegen unit, `panic = "abort"` and stripped symbols.

## Memory and CPU

Idle, running in the tray with no window open. Most of the memory belongs to the system web view, not to the Rust
program.

| | Windows | Linux |
|---|---|---|
| Processes | 8 (the app and 7 WebView2 processes) | 3 (the app, WebKitWebProcess, WebKitNetworkProcess) |
| Private memory / proportional (PSS) | 182 MB private | 207 MB PSS (app 64, web process 125, network process 17) |
| Working set / RSS | 418-428 MB | 398 MB |
| The app process alone | 32 MB working set | 64 MB PSS, 147 MB RSS |
| CPU in 60 s idle | 0.37 s (0.6 % of one core) | under 1 s² |
| After opening the flyout | no change | no change |

Working set and RSS count shared libraries once per process, so they overstate what the system really uses; the private
and PSS rows are the fairer figures. ² The Linux CPU time is read in whole seconds, so only an upper bound is known.
The flyout is created hidden at start-up, which is why opening it adds no memory. The Settings window is created when
you open it and its web view is freed when you close it.

## Speed

| Measurement | Windows | Linux | macOS |
|---|---|---|---|
| App start to command pipe ready | 427-535 ms (median 464 ms, 5 runs) | not measured | not measured |
| App start to first answered `budsctl status` | 489-612 ms (median 520 ms, simulated buds) | not measured | not measured |
| `budsctl status` through the running app (10 runs) | 29-37 ms (median 31 ms) | 7-10 ms (median 8 ms) | not measured |
| Opening the control channel with the real buds | 127-165 ms (3 runs) | 0.24 s (2026-11-16), about 0.3 s (2026-11-11) | 118-292 ms (2 runs) |
| Buds out of the case, to connected | not timed | about 15 s³ | not timed |

³ Linux: the app asks BlueZ to reconnect, then waits 3 s after the audio link is up before it opens the control
channel (opening earlier hangs); about 15 s in total. The Windows and macOS connections after the case worked, but were
not timed.

### Timings built into the app

These are constants in [`crates/buds-core/src/consts.rs`](../crates/buds-core/src/consts.rs).

| What | Value |
|---|---|
| Battery and noise-mode refresh | every 30 s |
| Device check and safety net | every 2 s |
| A request with no reply means the link is dead | 3 s |
| Retry delays after a failed connection | 1, 2, 3, 5, then every 8 s |
| After two failed opens in a row, the wait before the next one | 6 s, 12 s, doubling up to 5 min; "Reconnect" skips it |
| Wait after the buds join the computer, before opening the channel | 3 s |
| Timeout for one connection method | 5 s; 3 s for a fixed RFCOMM channel, and 3 s for everything on macOS |
| Equalizer change not confirmed by the buds | re-read after 1.2 s |
| Find my earbuds stops by itself after | 30 s |
| Ask the buds to join this computer, at most | once every 15 s |

## Project size

| | Files | Lines |
|---|---|---|
| Rust, `buds-core` (protocol, parsers, controller, settings, IPC) | 24 | 6,935 |
| Rust, `buds-bt` (Windows, Linux and macOS backends) | 10 | 2,477 |
| Rust, `src-tauri` (tray app) | 10 | 2,267 |
| Rust, `buds-art`, `budsctl`, `icongen` | 4 | 1,028 |
| Rust, generated device catalog | 1 | 959 |
| Svelte and TypeScript frontend | 69 | 3,835 |

Line counts include comments and the tests that sit next to the code. The 105-model device catalog is generated; see
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

| Tests | Count |
|---|---|
| Rust (`cargo test --workspace`, Windows) | 163 |
| Frontend (`npm test`) | 18 |
| **Total** | **181** |

Dependencies: 578 packages in `Cargo.lock` (every platform's, including the Tauri stack) and 103 in
`package-lock.json`, nearly all of them build-time tools (the app itself needs two npm packages). The app has no
network features and no updater; see [SECURITY.md](../SECURITY.md).

## Build times

| | Time |
|---|---|
| Windows, `package-windows.ps1 -SkipInstall`, warm Cargo cache (installer and zip) | 132 s |
| Linux, compiling the app in release mode (3 min 1 s) | about 3 min |

Both used an existing Cargo cache for most dependencies; a build from nothing takes longer. macOS build time was not
recorded.

## How these were measured

- **Sizes:** the files written by `scripts/package-*.sh` and `scripts/package-windows.ps1`, or by `tauri build`.
- **Memory and CPU, Windows:** the app and all of its child processes, sampled with PowerShell 6 s after start and again
  after 66 s; CPU time is the difference over 60 s.
- **Memory and CPU, Linux:** the app and its child processes; PSS and RSS from `/proc/<pid>/smaps_rollup`, CPU time
  from `ps`.
- **Start-up, Windows:** the time until the app's named pipe (`\\.\pipe\budscontrol-<user>`) exists, then until
  `budsctl status` is answered by the app.
- **Command speed:** ten consecutive `budsctl status` runs while the app was running, so each one was answered by the
  app and did not touch Bluetooth.
- **Connection times:** the timestamps in the app's own log, from "connecting" to "connected".

Numbers from one machine of each kind are a guide, not a benchmark. Reports from other hardware are welcome; see
[CONTRIBUTING.md](../CONTRIBUTING.md).
