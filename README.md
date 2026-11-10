# Buds Control

A tray app for switching **Noise Cancellation**, **Transparency** and **Off** on OnePlus, OPPO and realme earbuds
without the phone app. A Windows-first port (Linux and macOS to follow) of
[amarsakore/BudsControl](https://github.com/amarsakore/BudsControl), built with Rust + Tauri 2 and Svelte.

> **Status:** the Windows build is tested with OnePlus Buds 4. The Linux and macOS builds compile and pass the checks
> but have not been run on real machines yet (help wanted, see below). On a Mac the
> [original app](https://github.com/amarsakore/BudsControl) is the tested choice.
> See [PLAN.md](PLAN.md) and [docs/PROGRESS.md](docs/PROGRESS.md).

## Features

- One-click **noise control** (ANC, Transparency, Off) and the five **ANC levels** (Smart, Max, Moderate, Mild,
  Adaptive) where the model has them; battery for the left bud, right bud and case.
- **Tray icon** that shows the battery as a ring (with a charging bolt) and follows the Windows light/dark taskbar.
- **Equalizer** presets and custom EQ, **low latency** mode, **find my earbuds**, **touch gestures**.
- **Global shortcuts** (cycle / ANC / Transparency / Off) with a small on-screen popup.
- **Automatic reconnect** when the buds only joined your phone, optional **remembered settings**, **launch at
  login**.
- **`budsctl`**, a command-line tool that controls the buds directly, or asks the running app to.
- Developer tools: packet log, raw commands, service discovery.

105 models of the OnePlus Buds, OPPO Enco and realme Buds families are known; the feature set shown follows the model.

## Install (Windows 10 / 11, x64)

Download from the [Releases](https://github.com/nilcodes/BudsControl/releases) page. Pair your buds in
*Settings > Bluetooth & devices* first.

- **Installer** (`BudsControl-<version>-windows-x64-setup.exe`): installs for the current user, no administrator
  rights needed, adds a Start menu entry and an uninstaller. It needs the Microsoft Edge WebView2 runtime (already part
  of Windows 11 and up-to-date Windows 10; the installer downloads it if missing).
- **Portable** (`BudsControl-<version>-windows-x64-portable.zip`): unzip anywhere and run `BudsControl.exe`;
  `budsctl.exe` sits next to it.

The builds are **not code-signed**, so Windows SmartScreen shows "Windows protected your PC": choose **More info >
Run anyway**.

Windows hides new tray icons in the overflow ("^") next to the clock. To keep Buds Control visible, drag its icon
onto the taskbar, or turn it on under *Settings > Personalization > Taskbar > Other system tray icons*.

![The flyout](docs/screenshots/real-expanded-anc-dark.jpg)

Click the tray icon for the flyout; right-click for the menu (modes, Settings, Reconnect, Quit). Starting the
app again just opens the flyout.

## Linux (untested on hardware)

Needs BlueZ (`bluetoothd` running) and a desktop with a system tray (KDE, XFCE, Cinnamon, or GNOME with the AppIndicator
extension). Pair the buds in your desktop's Bluetooth settings first.

- **AppImage** (`BudsControl-<version>-linux-x64.AppImage`): `chmod +x` it and run. **.deb**
  (`BudsControl-<version>-linux-x64.deb`): `sudo apt install ./BudsControl-*.deb`.
- Linux trays don't report clicks, so everything starts from the tray menu: **Open Buds Control** opens the flyout at
  the mouse pointer (centred on Wayland).
- Mode changes made with a shortcut show a desktop notification instead of the popup. Global shortcuts work on X11
  only; on Wayland bind keys to `budsctl cycle` (or `anc`, `transparency`, `off`) in your desktop's keyboard settings.
- Buds Control talks to BlueZ over D-Bus, so you must be allowed to use Bluetooth (on some distributions that means
  being in the `bluetooth` group). `budsctl sdp` shows which services BlueZ knows for your buds.
- If you try it, please report what happened (`budsctl status`, the log at `~/.local/share/BudsControl/BudsControl.log`).

## macOS (untested on hardware)

macOS 11 or later, Apple silicon or Intel. Pair the buds in *System Settings > Bluetooth* first.

- Download `BudsControl-<version>-macos-arm64.dmg` (Apple silicon) or `-x64.dmg` (Intel) and drag the app to
  *Applications*. It is signed ad hoc, not notarised: the first time, right-click the app and choose **Open**
  (or run `xattr -dr com.apple.quarantine /Applications/BudsControl.app`).
- macOS asks once for permission to use Bluetooth; allow it. The app lives in the menu bar (no Dock icon): click the
  icon for the flyout, right-click or Control-click for the menu.
- The connection code follows the original app (the buds' "enco link" service record, the HeyMelody UUIDs, then
  channels 15, 12, 13), using IOBluetooth. `budsctl` works as on Windows.
- If you try it, please report what happened (`budsctl status`, the log at `~/Library/Logs/BudsControl.log`).

## Command line

```text
budsctl status                 show the buds' state
budsctl anc | transparency | off | cycle
budsctl max                    ANC level (smart, max, moderate, mild, adaptive)
budsctl eq=1   lowlatency=on   find=on   gesture=left,doubleTap,next
budsctl devices | sdp | audio  diagnostics (paired devices, services, audio endpoints)
```

The buds accept only one control connection, so when the tray app is running `budsctl` hands the command to it (named
pipe `\\.\pipe\budscontrol-<user>`) and prints the same output; `--direct` forces a direct connection. `budsctl
--help` lists everything, `--fake` tries the commands on simulated buds.

## How it works

The earbuds speak the HeyMelody protocol over a classic Bluetooth RFCOMM channel (service `079A`, channel 15 on the
Buds 4). Buds Control opens that channel with Winsock, asks for state and sends framed commands; see
[docs/PROTOCOL.md](docs/PROTOCOL.md). When the buds are connected only to your phone, a reconnect request to the
Windows Bluetooth audio driver (the same thing the volume flyout does) brings them to the PC. Nothing is sent to the
internet, and nothing is changed on the buds besides what you ask for.

## Development

Needs Rust (stable, MSVC toolchain on Windows), Node 20+ and the Visual Studio C++ Build Tools.

```powershell
npm install
npm run check          # svelte-check + tsc
cargo build --workspace
npm run tauri dev      # opens the app (set BUDS_FAKE=1 to use simulated buds)
```

Release builds go to `release/`: `./scripts/package-windows.ps1` (installer + portable zip);
`./scripts/package-linux.sh` (AppImage + .deb; needs `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev
librsvg2-dev libdbus-1-dev pkg-config patchelf libssl-dev libxdo-dev`); `./scripts/package-macos.sh` (.dmg for this
Mac's architecture). The app icon comes from
`cargo run -p icongen -- app-icon.png` followed by `npx tauri icon app-icon.png`.

Design review without the app or the buds: `npm run dev`, then open
`http://localhost:1420/flyout.html?scenario=<name>&more=1` in a browser (scenarios: connected, anc, customeq,
disconnected, connecting, lowbattery, charging, finding, longeq, minimal, noanc). A built-in simulator answers the
app's commands there. `npm test` runs the frontend unit tests.

Development switches: `BUDS_FAKE=1` runs the app against simulated buds, `BUDS_FAKE_SWEEP=1` (with it) sweeps the
battery level ten times a second, and `BUDS_TASKBAR_THEME=light|dark` forces the tray icon's colour. The icon
contact sheet is regenerated with `cargo run -p buds-art --example contact_sheet -- docs/screenshots/tray-icons.png`.

The original macOS app is cloned to `reference/upstream/` (gitignored) at commit `298d452` for porting.

## Credits

- [amarsakore](https://github.com/amarsakore) and contributors of the original BudsControl.
- Protocol references: the Noctalia `oppo-pods` plugin (MIT), Gadgetbridge's OPPO support, and RawS-Music's
  HeyMelody port (Apache-2.0). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Disclaimer

Unofficial project, not affiliated with or endorsed by OnePlus, OPPO, realme or Microsoft. Use at your own risk.

## License

[MIT](LICENSE)
