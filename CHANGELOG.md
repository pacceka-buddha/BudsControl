# Changelog

All notable changes to this project are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.0.0] - 2026-11-18

First public release.

### Added

- Tray app for Windows 10/11, Linux and macOS with a flyout: noise control (ANC, Transparency, Off), the ANC levels,
  and battery for both buds and the case.
- Tray icon that shows the battery level and charging state and follows the light or dark taskbar.
- Equalizer presets and custom EQ, low latency mode, find my earbuds, and touch gesture settings.
- Global shortcuts with a small on-screen popup (not on Linux/Wayland, where the desktop does not allow it; see
  [docs/LINUX.md](docs/LINUX.md)).
- Automatic reconnect when the buds joined only your phone, remembered settings, and launch at login.
- `budsctl`, a command-line tool that controls the buds directly or through the running app.
- Developer tools: packet log, raw commands, and service discovery.
- A catalog of 105 OnePlus, OPPO and realme models; the controls shown follow what the model supports.
- Packages built by GitHub Actions: Windows installer and portable zip, Linux AppImage and `.deb`, and macOS `.dmg` for
  Apple silicon and Intel.

### Notes

- Tested on real hardware: Windows 10/11 with OnePlus Buds 4; Ubuntu 24.04 with GNOME (X11 and Wayland) and Buds 4,
  including the `.deb` and AppImage; one Apple silicon Mac on macOS 27 with OnePlus Buds 3, including the `.dmg`.
  Intel Macs, other Linux distributions and desktops, and other earbud models are untested. Results are in
  [docs/TESTING.md](docs/TESTING.md).
- The builds are not code-signed or notarised. Windows SmartScreen and macOS Gatekeeper warn on the first launch; the
  [README](README.md#download-and-install) explains how to get past that.
- Known issue on macOS: the first connection from a fresh `budsctl` process sometimes hangs for 3 s before the retry
  connects. It was not seen in the app, and its cause is unknown.
- Figures for size, memory and speed are in [docs/PERFORMANCE.md](docs/PERFORMANCE.md).
