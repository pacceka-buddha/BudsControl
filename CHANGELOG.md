# Changelog

All notable changes to this project are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- After the buds came out of the case, the app could stay on "Connected" with no battery and no controls. The control
  service sometimes timed out right after the buds reconnected, so the app fell back to the serial port service, which
  accepts a connection but sends data the app cannot read. Because any received bytes counted as an answer, the serial
  port was then remembered and used first on every later start. Now a connection method is remembered only after the
  buds send a valid reply, and the remembered method is dropped when the buds stop answering.
- Switching the EQ preset is steadier: a change waits for the buds to confirm it, stale readings while they switch are
  ignored, and the app re-reads after 1.2 s if no confirmation comes. An EQ change made on the buds themselves (stem
  gesture, phone app) now shows up at once (`0x0504` push). Both follow upstream 1.3.4.
- When the control channel can't be opened, the app now waits longer between tries (6 s, 12 s, up to 5 min) after two
  failures in a row, instead of retrying every 8 s forever. Asking to reconnect skips the wait. Follows upstream 1.3.5.
- macOS: `budsctl` could not hand commands to the running app, so the two fought over the buds' single control channel.
  The app's command socket path was longer than macOS's 104-byte limit and was never created; a short fallback path
  is now used when the usual one does not fit.
- A control-channel timeout on the remembered connection method no longer falls through to the serial port (channel
  12). On OnePlus Buds 3 that channel opens but the buds never answer, which showed as a blank status and `-1%`
  battery; the app now retries the right channel instead.
- macOS: a control channel that does not open is given up on after 3 s instead of 5 s (as the original app does), so
  the first connect is about 2 s quicker.
- Windows: listing the paired devices gives up after 15 s, so a hung Bluetooth call can't leave the app stuck at start-up.
- Linux: after the buds dropped, the app reconnected in a tight loop (about 25 open/close cycles in 300 ms) instead of
  waiting between attempts. A failed write now follows the normal retry timer.
- Linux (X11): the flyout opened far from the panel and stayed at the wrong height after collapsing. It now sits
  against the panel edge (bottom, top or side panel) and stays there when it grows or shrinks.

### Changed

- Linux (Wayland): the app starts through Xwayland when it can, so the flyout can be placed. It opens at the right end
  of the panel instead of the top-left corner. Set `GDK_BACKEND=wayland` to run natively. See
  [docs/LINUX.md](docs/LINUX.md) for the Wayland limitations (no global shortcuts, flyout not under the icon).
- macOS: a theme test that assumed a dark taskbar override now accepts macOS's always-light template icon, so the
  workspace tests, clippy and rustfmt pass on macOS too.

- Docs: Linux and macOS were run on real hardware (Ubuntu 24.04 with a OnePlus Buds 4, macOS 27 on Apple silicon with
  a OnePlus Buds 3); the results are in [docs/TESTING.md](docs/TESTING.md).
- Docs: new [docs/PERFORMANCE.md](docs/PERFORMANCE.md) with package sizes, memory, CPU, start-up and connection times.
- Docs: the macOS install steps now use **Open Anyway** in Privacy & Security.

## [0.1.0] - 2026-11-11

First public release.

### Added

- Tray app for Windows 10/11 with a flyout: noise control (ANC, Transparency, Off), ANC levels, battery for both buds
  and the case.
- Tray icon that shows the battery level and charging state and follows the light/dark taskbar.
- Equalizer presets and custom EQ, low latency mode, find my earbuds, touch gesture settings.
- Global shortcuts with an on-screen popup.
- Automatic reconnect, remembered settings, launch at login.
- `budsctl` command-line tool; it talks to the running app when there is one.
- Developer tools: packet log, raw commands, service discovery.
- Catalog of 105 OnePlus, OPPO and realme models.
- Linux (BlueZ) and macOS (IOBluetooth) support. These compile and pass the checks but have not been tested on real
  hardware yet.
- Windows installer and portable zip, Linux AppImage and .deb, macOS .dmg, built by GitHub Actions.
