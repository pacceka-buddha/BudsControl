# Changelog

All notable changes to this project are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- Linux: after the buds dropped, the app reconnected in a tight loop (about 25 open/close cycles in 300 ms) instead of
  waiting between attempts. A failed write now follows the normal retry timer.
- Linux (X11): the flyout opened far from the panel and stayed at the wrong height after collapsing. It now sits
  against the panel edge (bottom, top or side panel) and stays there when it grows or shrinks.

### Changed

- Linux (Wayland): the app starts through Xwayland when it can, so the flyout can be placed. It opens at the right end
  of the panel instead of the top-left corner. Set `GDK_BACKEND=wayland` to run natively. See
  [docs/LINUX.md](docs/LINUX.md) for the Wayland limitations (no global shortcuts, flyout not under the icon).

### Tested

- Linux: first run on real hardware (Ubuntu 24.04, GNOME, OnePlus Buds 4). Results in `docs/TESTING.md` (Gate 11).

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
