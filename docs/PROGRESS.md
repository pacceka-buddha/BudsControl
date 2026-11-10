# Progress

Plan: [../PLAN.md](../PLAN.md). Update this file at the end of every phase with the gate evidence.

## Pinned versions (Phase 0)

| Item | Version |
|---|---|
| Rust | 1.99.1 stable, `x86_64-pc-windows-msvc` (edition 2024) |
| MSVC Build Tools | VS 2022 17.14.44, MSVC 14.44.35207 |
| tauri / tauri-build / @tauri-apps/cli, api | 2.12.5 / 2.7.4 / 2.12.4 |
| Svelte / Vite / vite-plugin-svelte | 5.57.3 / 8.3.7 / 7.3.1 |
| TypeScript | 6.0.x |
| Node / npm | 24.19 / 11.17 |

Notes on differences from PLAN.md:
- Vite 8 uses Rolldown: the multi-page input is configured under `build.rolldownOptions` (not `rollupOptions`).
- Upstream reference is cloned at `reference/upstream` (gitignored), pinned to `298d452`.
- `npm audit` reports 2 low advisories from `deepmerge` via `@sveltejs/vite-plugin-svelte` (dev-only, no fix available).

## Phase 0: toolchain and skeleton

Done. Gate 0 evidence:

- `cargo build --workspace` OK (MSVC links; first build 2m20s).
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` OK (no tests yet).
- `npm run check`: 0 errors, 0 warnings. `npm run build` emits `dist/{flyout,settings,hud}.html`.
- `npm run tauri dev`: `budscontrol.exe` runs, a visible "Buds Control" window loads `flyout.html` via WebView2.
- `npm run tauri build -- --debug` produces `target/debug/bundle/nsis/Buds Control_0.1.0_x64-setup.exe` (2.46 MiB).
- Bundle identifier is the placeholder `io.github.nilcodes.budscontrol` (change in `src-tauri/tauri.conf.json` if wanted).
- Icons are still the Tauri defaults; Phase 10 generates the real ones.

## Phase 1: protocol, parsers, catalog

Done. Gate 1 evidence:

- `cargo test -p buds-core`: 39 tests in `crates/buds-core/tests/gate1.rs` pass, plus 14 generated ts-rs export tests.
  Covered: frame encode (`AA 07 00 00 06 01 01 00 00`), parser (one frame, 3 chunks, two frames, junk, 2-byte LEB128,
  truncated tail, malformed lengths, noise fuzz), battery/capabilities/product ID/wear/switches/EQ/gestures/hosts
  decoders (and "never panic on any prefix"), Buds 4 + Pro 3 noise bits, set-noise masks, custom EQ round trip,
  catalog size (105, sorted, unique) and fallbacks (generic, realme).
- `cargo clippy --workspace --all-targets -- -D warnings` and `npm run check` clean.
- `tools/convert_catalog.py` regenerates `catalog/generated.rs` from the pinned upstream file (105 models; it aborts
  if the parsed count differs from the entries in the Swift file).
- TypeScript types are exported to `src/lib/bindings/` by `cargo test` (`.cargo/config.toml` sets `TS_RS_EXPORT_DIR`).

Deviations from the Swift code (malformed input only): the frame parser treats a length varint longer than 4 bytes
or a length above 65535 as noise instead of waiting forever. Documented in `docs/PROTOCOL.md`.

## Phase 2: controller actor + fake buds

Done. Gate 2 evidence:

- `cargo test -p buds-core`: 28 scenario tests in `tests/controller.rs` (tokio paused clock, so retry delays run
  instantly and deterministically; repeated 5x, stable), 7 in `tests/support.rs` (settings, packet log), plus the
  Phase 1 tests. `cargo clippy --workspace --all-targets -- -D warnings`, `npm run check` clean.
- Covered: connect + read state; ANC level choice (last -> settings -> first); optimistic UI and a stale noise reply
  not flipping it back; cycle order; mode changed on the buds; mode chosen while disconnected sent after the
  product ID; low latency / EQ / custom EQ clamping / gestures; find-my-earbuds auto-stop at 30 s; retry delays
  1,2,3,5,8,8 s; 3 s reply watchdog; connecting watchdog; failed open; buds into the case and out again;
  3 s settling after the buds appear; page rate limit (15 s) and `autoConnect`; paging that brings the buds back;
  resume from sleep; different buds reset state; remembered settings (restore only when different, off by default,
  low latency recorded); shutdown; every frame logged; CLI `status`/actions/`raw`/`--scan`/`--watch` output.
- `cli::execute` reproduces the upstream status block and accepts the upstream spellings (`--cli anc`, `--watch 30`).
  `sdp` is a stub until Phase 9.
- CI (`.github/workflows/ci.yml`): fmt, clippy and tests for `buds-core` on Windows/Ubuntu/macOS, a check that the
  generated TypeScript bindings are committed, and the frontend check/build. Not run yet (no remote).

Design notes and deviations from the Swift code:
- Device events (connected, disconnected, wake from sleep) are polling-based (every 2 s) as planned. The health
  poll's "buds connected but not us" safety net only fires after the buds have been connected for 3 s, so it can't
  race the delayed connect.
- Fix: remembered settings are only compared with a reading from the *current* link. The Swift code kept the
  pre-disconnect mode, so the low-latency reply could consume the restore on a stale value (found by the tests).
- A new log line "retrying in Ns" is written on every retry (useful in the packet log; the tests rely on it).
- `Platform::open` takes the last working candidate as `hint`, and `Platform::wall_time` is overridable so tests can
  simulate sleep.

## Phase 3: Windows Bluetooth backend + budsctl

**Code complete; Gate 3 (real Buds 4) is PENDING** - see `docs/TESTING.md`, section "Gate 3". Not marked done.

What exists:
- `crates/buds-bt`: `plan.rs` (OS-independent candidate walk: last working candidate, HeyMelody 079A, 1107, SPP only if
  both "not found", channels 15/12/13 only if all "not found"; a timeout or host-down on a service ends the walk;
  9 unit tests) and `win/` (device list via `BluetoothFindFirstDevice` with no inquiry; RFCOMM through Winsock
  `AF_BTH`: connect with a per-candidate timeout on its own thread, cancellable when the controller drops the
  open, reader thread that owns closing the socket, peer channel via `getpeername`, page = connect to 079A and
  drop). Directory is `win/` rather than the plan's `windows/` to avoid clashing with the `windows` crate name.
- `crates/budsctl`: `budsctl [--fake] [--direct] <command>` with every CLI command of `buds_core::cli`, plus
  `devices` (paired devices). Real runs share `%APPDATA%\BudsControl\settings.json` and log to
  `%LOCALAPPDATA%\BudsControl\BudsControl.log`. `--direct` is accepted but is the only mode until Phase 9.
- `buds_core::paths` (settings/log locations) and `platform::name_looks_like_buds`.

Verified without the buds being connected:
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `npm run check` clean.
- `budsctl --fake status|max|devices` print the expected output.
- Real Bluetooth, read-only: `budsctl devices` lists "OnePlus Buds 4" (paired, not connected) and a Sony WH-CH720N;
  `budsctl status` with the buds out of reach pages them, times out after 5 s, backs off 1,2,3,5 s, prints the
  not-connected block and exits 1. The log file is written.

Not verified yet (needs the connected buds): which candidate connects, replies, mode switching, etc.

### Gate 3 hardware run, 2026-11-10 (OnePlus Buds 4, read-only part, run by Claude)

- **Connect candidate: HeyMelody service 079A**, found by Windows' own SDP lookup; connected in ~360 ms (`connected via HeyMelody service 079A`).
  Stored as connect hint `heymelody` in `settings.json`. The fallbacks (1107, SPP, channels) were not needed.
- `budsctl status`: `OnePlus Buds 4 [065414]`, L/R 100%, mode read correctly (transparency, reported as bit 8 as the
  catalog predicts), EQ preset, low latency, real gestures, 35 capability bits, `shows:` all six features.
- `raw 0106` gets the `06 81` reply; `watch` shows pushes and the multipoint list (this PC + the phone).
- Case battery is not reported while the buds are out of the case (`case -1%`), the UI will show "-".
- Still for the owner (audible / physical): 3.3-3.5 (mode and level switching, watching pushes while using the buds),
  3.7-3.11 (EQ, custom EQ, low latency, find, gesture), 3.12 (buds in the case -> exit 1).

### Gate 3, second hardware run, 2026-11-10 (run by Claude; original settings restored afterwards)

Passed, each change read back by a fresh `budsctl` session: Off / Transparency / ANC, every ANC level (Smart bit 7,
Max 4, Moderate 5, Mild 6, Adaptive 11; Off reads back as bit 3, Transparency as 8), EQ presets 1 / 2 / 0, custom EQ
band write (Custom1, then reset to flat), low latency on/off, gesture write (left double tap next, then restored).
Final state equals the starting state (Transparency, preset 0, low latency off, left previous / right next).

Still open, needs the owner: 3.5 (pushes while using the buds), 3.10 (find, loud: buds out of ears), 3.11 (a real
double tap), 3.12 (buds in the case -> exit 1), and confirming by ear that the modes audibly change.

## Phase 4: Tauri shell (controller, tray, flyout, bridge)

Done, with two items left for the owner (below). Gate 4 evidence (Windows 11, taskbar at the bottom, 100% scale,
checked visually with screenshots; `BUDS_FAKE=1` and the real Buds 4):

- Tray icon appears (inside the hidden-icons popup here); left click opens the flyout above the icon, rounded
  corners and shadow, 300 px wide, no taskbar entry. Esc and a click elsewhere hide it.
- Debug flyout shows the live snapshot JSON; the ANC button changed the mode (Smart) and the tray tooltip followed:
  `Buds Control · OnePlus Buds 4 / L 80% · R 82% · Case 45% / ANC · Smart`. With the real buds the snapshot shows
  `connected`, `065414`, L/R 100%, transparency, bit 8.
- A second launch exits at once and shows the running instance's flyout (one process stays).
- Settings button opens the settings window (still the Phase 0 skeleton) and now closes the flyout first.
- Unit tests: placement for bottom / top / left / right taskbars, clamping, negative-coordinate monitors, anchored
  growth (9 tests); tooltip text and the 127-character cut (4); accent colour conversion.

Not verified yet (needs the owner):
- The right-click tray menu (mode check marks, Settings, Reconnect, Quit): the screen-control tool can't right-click.
- Other geometry: taskbar on another edge, 150% scaling, a second monitor (the placement maths is unit-tested).

Notes and deviations:
- Module is `app_windows` (a `windows` module would clash with the `windows` crate).
- Plugins: only single-instance for now; global-shortcut (Phase 8), autostart (Phase 7), clipboard (Phase 7) are
  added with the phases that use them. Tray icon is the Tauri default until Phase 6.
- Window effect: Acrylic is requested; it is not visibly translucent against the debug page's opaque background,
  which Phase 5 replaces. Revisit there.
- `channel` is `null` in the snapshot on Windows: `getpeername` does not return the SDP-resolved RFCOMM channel.
  Cosmetic (Developer tab); investigate in Phase 9 together with `budsctl sdp`.
- `BUDS_FAKE=1` runs the app against the simulated buds; settings and log are then in memory only.

## Phase 5: flyout UI

Done, with the limits listed below. Gate 5 evidence:

- `npm run check` 0 errors, `npm test` (8 tests for the formatting / geometry helpers), `npm run build`, clippy and all
  Rust tests clean.
- Built: tokens (light/dark from `prefers-color-scheme`, Windows accent applied at runtime), `Segmented` (sliding
  accent pill that follows animated widths; radio-group keyboard model), `BatteryRing`, `EarbudsGlyph`, `Icon`
  (own line icons), `Card`, `Row`, `ToggleSwitch`, `PickerButton` + native menus (`menu.ts`), `Button`, `Spinner`,
  `BandSlider` / `EqBands` (pointer + keyboard, sent on release), and the flyout sections: header, modes, ANC
  levels, battery row, More panel (EQ buttons or menu, custom EQ, low latency, find, gestures), disconnected /
  connecting panel. The window follows the content height (opening grows it first, closing shrinks it after).
- Visual review in every state with the browser simulator (`src/lib/mock.ts`), light and dark: connected, ANC with
  levels, custom EQ, disconnected, connecting, low battery (red), charging (green bolts), long EQ names (menu).
  Screenshots: `docs/screenshots/`.
- Interaction checks in the simulator: radio keyboard model (roving tab stop, arrows), level row appears only in
  ANC, switch, reported window height equals the content height.
- **Real window + real Buds 4 (driven through the UI):** collapsed and expanded flyout with Acrylic over a changing
  wallpaper (stays readable), ANC with the last-used level, level change, More panel, EQ preset menu (native
  Windows menu with a check mark, flyout stays open behind it), Custom1 sliders (drag the 1 kHz band to +3, reset
  to 0), low latency on/off, gesture menu (Next track, restored), Transparency, header "..." menu, Settings opens
  the settings window and closes the flyout. The buds were returned to their original settings.

Deviations and limits:
- Icons are drawn for this app instead of Fluent UI System Icons (their path data can't be reproduced reliably
  from memory); the Fluent entry was removed from THIRD_PARTY_NOTICES.
- The window is the flyout (translucent surface over Acrylic, DWM rounds the corners) instead of a card inside a
  transparent window, so a resizing window never shows a mismatched strip.
- Not verified: 150% / 200% display scaling, the light theme in the real window (the PC is on dark; light was
  reviewed in the browser simulator), taskbar on other edges, find-my-earbuds (loud, deliberately not played),
  a second monitor.
- While the app runs, `budsctl` can't talk to the buds (one control connection): Phase 9 adds the IPC.
- Launching a second instance can show the flyout and have it hide again at once (the launching process takes the
  focus); a tray click is the normal path.

## Phase 6: dynamic tray icon, tooltip, menu state

Done. Gate 6 evidence:

- `buds-art`: tiny-skia drawing of the earbuds glyph (same geometry as the app icon) inside a battery ring: arc
  clockwise from 12 o'clock over a 28% track, glyph at 45% and no arc when disconnected, white on a dark taskbar and
  black on a light one. 7 pixel-level unit tests (sizes, arc coverage at several angles, full / empty / unknown,
  dimming, colour, transparent corners and left-right balance, straight alpha). The tests caught a real artefact:
  translucent head and stem shapes stacked into a brighter joint, fixed by drawing the glyph as an opaque layer
  and compositing it with the opacity.
- Contact sheet `docs/screenshots/tray-icons.png`: every state at 16 / 20 / 24 / 32 px on dark and light, enlarged
  8x without smoothing. Crisp at all four sizes (16 px is the busiest, still readable).
- App: icon size is `round(16 x primary-monitor scale)` (16 / 20 / 24 / 32 at 100 / 125 / 150 / 200%), the colour
  follows `SystemUsesLightTheme`, and it is redrawn only when level, link, size or colour change; every state
  change and a 10 s timer re-check it. Unit tests for the spec (scale, disconnected, clamping).
- On screen (100% scaling): the tray shows the ring icon instead of the Tauri logo; with `BUDS_TASKBAR_THEME=light`
  the same icon turned black next to the light-on-dark neighbours.
- Leak check: `BUDS_FAKE_SWEEP=1` changed the battery level ten times a second for 3 minutes (about 1,800 icon
  redraws). GDI objects stayed at 20, USER objects at 34, handle count and working set (37 MB) were flat.
  (The plan asked for 10 minutes of normal updates; this is a much heavier workload.)

Not verified (needs the owner or other hardware): the icon at 125 / 150 / 200% on a real display, an actual switch of
the Windows taskbar theme (the colour logic is exercised through the override), and the right-click menu's check
marks and enabled state (the screen-control tool can't right-click; the code has been in place since Phase 4).
