<p align="center">
  <img src="src-tauri/icons/128x128.png" alt="" width="96" height="96">
</p>

<h1 align="center">Buds Control</h1>

<p align="center">
  <b>Control OnePlus, OPPO and realme earbuds from the tray, without the phone app.</b><br>
  Windows · Linux · macOS
</p>

<p align="center">
  <a href="../../releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/pacceka-buddha/BudsControl?label=release"></a>
  <a href="../../actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pacceka-buddha/BudsControl/ci.yml?branch=main&label=CI"></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey">
</p>

<p align="center">
  <a href="#download-and-install">Download</a> ·
  <a href="#using-it">Using it</a> ·
  <a href="#command-line-budsctl">Command line</a> ·
  <a href="#troubleshooting">Troubleshooting</a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

<p align="center">
  <img src="docs/screenshots/flyout-windows-anc.jpg" alt="The flyout on Windows 11 with noise cancellation on" height="430">
  <img src="docs/screenshots/flyout-windows-eq.jpg" alt="The flyout on Windows 11 with the custom equalizer open" height="430">
  <img src="docs/screenshots/linux-flyout-expanded.jpg" alt="The flyout on Ubuntu 24.04 with GNOME" height="430">
</p>

Switch between noise cancellation, transparency and off in one click, pick the ANC level, check the battery and change
the equalizer. Buds Control is a cross-platform port of
[amarsakore/BudsControl](https://github.com/amarsakore/BudsControl), a macOS app, built with Rust, Tauri 2 and Svelte.
It is small, works offline and sends nothing to the internet.

## Features

- **Noise control**: ANC, Transparency and Off in one click, plus the five ANC levels (Smart, Max, Moderate, Mild,
  Adaptive) on models that have them.
- **Battery** for the left bud, right bud and case, also drawn on the **tray icon** as a ring split in two (left half
  for the left bud, right half for the right, with a charging bolt) that follows the light or dark taskbar.
- **Equalizer** presets and custom EQ, **low latency** mode, **find my earbuds** and **touch gestures**.
- **Global shortcuts** (cycle, ANC, Transparency, Off) with a small on-screen popup.
- **Automatic reconnect** when the buds joined only your phone, optional **remembered settings**, **launch at
  login**.
- **`budsctl`**, a command-line tool that controls the buds directly or through the running app.
- **Developer tools**: packet log, raw commands and service discovery.

105 models of the OnePlus Buds, OPPO Enco and realme Buds families are known; the controls shown follow what the model
supports.

## Platform support

| Platform | Status | Tested with |
|---|---|---|
| **Windows 10 / 11** (x64) | Tested | OnePlus Buds 4 |
| **Linux** (x64) | Tested on Ubuntu 24.04; other distributions and desktops are **untested** | OnePlus Buds 4, GNOME on X11 and Wayland, `.deb` and AppImage |
| **macOS 11+** (Apple silicon, Intel) | Tested on one Apple silicon Mac; Intel Macs are **untested** | OnePlus Buds 3, macOS 27, `.dmg` install |

Hardware reports from untested setups are welcome; see [Contributing](#contributing). Wayland limits are in
[docs/LINUX.md](docs/LINUX.md). On a Mac, the [original app](https://github.com/amarsakore/BudsControl) is the
long-tested choice.

## Footprint

| | Windows | Linux | macOS |
|---|---|---|---|
| Download | 1.8 MiB installer, 2.5 MiB portable zip | 4.0 MiB `.deb`, 88 MiB AppImage | 3.2 MiB `.dmg` (Apple silicon) |
| Memory while idle in the tray | 182 MB private (app and web view) | 207 MB proportional (app and web view) | not measured |
| CPU while idle | 0.6 % of one core | under 1 s of CPU per minute | not measured |
| Connecting to the buds | 0.13-0.17 s | 0.24-0.3 s | 0.12-0.29 s |

Most of the memory is the system web view that draws the window; the Rust program itself is 32-64 MB. How these were
measured, and more numbers: [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Download and install

Get the latest build from the [**Releases**](../../releases/latest) page. Pair your earbuds with the computer in the
system's Bluetooth settings **before** starting Buds Control.

### Windows

| File | What it is |
|---|---|
| `BudsControl-<version>-windows-x64-setup.exe` | Installer. Installs for the current user (no administrator rights), adds a Start menu entry and an uninstaller. |
| `BudsControl-<version>-windows-x64-portable.zip` | Portable. Unzip anywhere and run `BudsControl.exe`; `budsctl.exe` sits next to it. |

- Needs the Microsoft Edge **WebView2** runtime (part of Windows 11 and up-to-date Windows 10; the installer
  downloads it if missing).
- The builds are **not code-signed**, so SmartScreen shows "Windows protected your PC": choose **More info > Run
  anyway**.
- Windows hides new tray icons behind the **^** arrow next to the clock. To keep Buds Control visible, drag its icon
  onto the taskbar, or turn it on under *Settings > Personalization > Taskbar > Other system tray icons*.

### Linux

| File | What it is |
|---|---|
| `BudsControl-<version>-linux-x64.AppImage` | Runs on most distributions: `chmod +x` it and start it. |
| `BudsControl-<version>-linux-x64.deb` | Debian / Ubuntu: `sudo apt install ./BudsControl-*.deb` |

- Needs BlueZ (`bluetoothd` running) and a desktop with a system tray (KDE, XFCE, Cinnamon, or GNOME with the
  AppIndicator extension).
- Buds Control talks to BlueZ over D-Bus, so your user must be allowed to use Bluetooth (on some distributions that
  means being in the `bluetooth` group).
- More in [docs/LINUX.md](docs/LINUX.md).

### macOS

- Download `BudsControl-<version>-macos-arm64.dmg` (Apple silicon) or `BudsControl-<version>-macos-x64.dmg` (Intel)
  and drag the app to *Applications*.
- The app is signed ad hoc, not notarised, so macOS says it could not verify it. Click **Done** (not Move to Bin), open
  **System Settings > Privacy & Security**, scroll down and click **Open Anyway** next to Buds Control (on older macOS
  versions, right-click the app and choose **Open**). Or run
  `xattr -dr com.apple.quarantine "/Applications/Buds Control.app"` once.
- macOS asks once for permission to use Bluetooth; allow it.

## Using it

| | Windows | Linux | macOS |
|---|---|---|---|
| Open the flyout | click the tray icon | tray menu > **Open Buds Control** | click the menu-bar icon |
| Menu (modes, Settings, Reconnect, Quit) | right-click the tray icon | click the tray icon | right-click or Control-click |
| Shortcut popup | on-screen popup | desktop notification | on-screen popup |

Starting the app a second time just opens the flyout. Shortcuts are set under **Settings > Shortcuts**. The earbuds only
accept a noise-mode change while they are in your ears.

**Linux and Wayland.** Global shortcuts work on X11 only. On Wayland, bind keys to `budsctl cycle` (or `anc`,
`transparency`, `off`) in your desktop's keyboard settings instead. The app also runs through Xwayland there so it can
place the flyout, which opens at the right end of the panel instead of under the tray icon. Details and other known
issues: [docs/LINUX.md](docs/LINUX.md).

## Command line (`budsctl`)

`budsctl` ships with the app (next to `BudsControl.exe`, or inside the app bundle).

```text
budsctl status                 show the buds' state
budsctl anc | transparency | off | cycle
budsctl max                    ANC level (smart, max, moderate, mild, adaptive)
budsctl eq=1   lowlatency=on   find=on   gesture=left,doubleTap,next
budsctl devices | sdp | audio  diagnostics (paired devices, services, audio endpoints)
```

The buds accept only one control connection, so while the tray app is running `budsctl` hands the command to it and
prints the same output; `--direct` forces a direct connection. `budsctl --help` lists everything, and `--fake` tries
the commands on simulated buds.

## Troubleshooting

- **"Not connected"**: make sure the buds are out of the case and connected to *this* computer (not only to your
  phone). Use **Reconnect** in the menu, or turn on *Reconnect buds to this PC automatically* in Settings.
- **"Connected" but no battery or controls**: the buds accept only one control connection. If your phone's earbud app
  or another program is holding it, Buds Control can see the buds but gets no answers. Close the other app or turn off
  the phone's Bluetooth; the app retries by itself (**Reconnect** skips the wait).
- **A mode change is ignored**: the earbuds only accept a noise-mode change while they are in your ears. Some models
  also lack certain modes or levels; the flyout shows only what the model reports.
- **Shortcut shows "Used by another app"**: another program has registered that key combination; pick another.

**Collecting details for a bug report.** Run `budsctl status` and `budsctl devices`, and attach the log file:

| OS | Log file |
|---|---|
| Windows | `%LOCALAPPDATA%\BudsControl\BudsControl.log` |
| Linux | `~/.local/share/BudsControl/BudsControl.log` |
| macOS | `~/Library/Logs/BudsControl.log` |

The log contains your Bluetooth device names and addresses; remove them before posting if you prefer.

## How it works

The earbuds speak the HeyMelody protocol over a classic Bluetooth RFCOMM channel (service `079A`, channel 15 on the
Buds 4). Buds Control opens that channel (Winsock on Windows, BlueZ on Linux, IOBluetooth on macOS), asks for the
state and sends framed commands. When the buds are connected only to your phone, a reconnect request to the system's
Bluetooth audio driver brings them to the computer. Nothing is changed on the buds besides what you ask for.

Details: [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Building from source

Requirements: Rust 1.90 or newer (the MSVC toolchain and the Visual Studio C++ Build Tools on Windows) and Node 20.19
or newer, or 22.12 or newer. On Linux also: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
libdbus-1-dev pkg-config patchelf libssl-dev libxdo-dev`.

```bash
npm install
npm run tauri dev          # run the app (set BUDS_FAKE=1 to use simulated buds)
```

Checks:

```bash
npm run check              # svelte-check + tsc
npm test                   # frontend unit tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Release packages go to `release/`:

| OS | Command | Output |
|---|---|---|
| Windows | `./scripts/package-windows.ps1` | installer + portable zip |
| Linux | `./scripts/package-linux.sh` | AppImage + .deb |
| macOS | `./scripts/package-macos.sh` | .dmg for the Mac's architecture |

Pushing a `v*` tag builds all of them on GitHub Actions and attaches them to a release; see
[docs/RELEASING.md](docs/RELEASING.md). The crate layout, working without earbuds and regenerating assets are covered
in [CONTRIBUTING.md](CONTRIBUTING.md#project-layout-and-tools).

## Contributing

Bug reports, hardware reports (especially from **Linux** and **macOS**, and from earbud models other than the Buds 4)
and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## Credits

- [amarsakore](https://github.com/amarsakore) and the contributors of the original
  [BudsControl](https://github.com/amarsakore/BudsControl).
- Protocol references: the Noctalia `oppo-pods` plugin (MIT), Gadgetbridge's OPPO support, and RawS-Music's HeyMelody
  port (Apache-2.0). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Disclaimer

Unofficial project, not affiliated with or endorsed by OnePlus, OPPO, realme, Microsoft or Apple. All product names
are trademarks of their owners. Use at your own risk.

## License

[MIT](LICENSE)
