# Progress

Plan: [PLAN.md](PLAN.md). Update this file at the end of every phase with the gate evidence.

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
- Bundle identifier: `io.budscontrol.desktop` (in `src-tauri/tauri.conf.json`).
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

## Phase 7: settings window + launch at login

Done, with the items below left for the owner. Gate 7 evidence (real app, real Buds 4, Windows 11):

- Settings window (600 x 640, Mica backdrop with a plain fallback, created on demand, destroyed on close): General,
  Shortcuts, Developer (tab only while "Show developer options" is on, and it returns to General if that is turned
  off). Built from new `SettingRow` / `Tabs` components; light and dark reviewed in the browser simulator
  (`settings.html?tab=...&dev=1`), dark and the real data on screen.
- General: device card (ring, name, "Connected · model 065414 · 100% · Off", Reconnect), Launch at login,
  Reconnect automatically, Remember settings, Show popup on shortcut, Show developer options, tip, version.
- Persistence: toggled Remember settings and Developer options on, restarted the app, reopened Settings: both still
  on, and the flyout's More panel state was restored. `update_settings` merges a partial object (pure `apply_patch`,
  3 unit tests: partial patch, unknown keys ignored / wrong types rejected, maps replaced whole); the controller reads
  the shared store at use time, so changes apply immediately.
- Launch at login (tauri-plugin-autostart, args `--background`): turning it on created
  `HKCU\...\Run\Buds Control = ...\budscontrol.exe --background`; turning it off removed it. The toggle reads the
  system state each time the page opens and when the window regains focus.
- Developer tab on the real buds: device facts, every noise-mode bit with Apply / check mark, game mode switch,
  paired devices (this PC + the phone), raw command (hex validation disables Send), quick buttons, scan, and the live
  packet log (Pause, Copy -> "Copied" through the clipboard plugin, Clear; follows new lines unless scrolled up;
  600-line cap). The log showed the buds rejecting an Apply with `mode change rejected: 0E` and the UI correctly
  stayed on the real mode.
- Closing the window frees its webview: 8 processes / 487 MB with it open, 7 / 402 MB after (dev build).
- Frontend: 11 vitest tests (new: hex command parsing, log clock), `npm run check` clean. Screenshots in
  `docs/screenshots/`.

Left for the owner / noted:
- Sign out and back in to see the autostart entry start the app hidden (the registry entry and its `--background`
  argument are verified; the dev build registers `target\debug\budscontrol.exe`).
- Shortcuts tab is a placeholder until Phase 8 (lists the four actions and any saved combination).
- The buds answered the noise-mode command with status `0x0E` (rejected) in their current state at the time of
  testing (mode Off, case battery not reported). Earlier phases switched modes fine, so this looks like the buds
  refusing changes in some physical state (in / near the case?). Worth re-checking with the buds worn or out of the
  case; the app's handling (re-read the real mode) is correct either way.
- Wear status: reply 0x0109 on this firmware carries values 7 / 5 / 4 rather than 0 / 1 / 3, so the parsed wear
  state is empty. Not shown anywhere yet; revisit in Phase 9 if wear detection is wanted.
- RFCOMM channel still shows "-" (see Phase 4 notes).
- Settings and the registry were restored to their previous values after testing.

## Phase 8: global shortcuts + HUD

Done. Gate 8 evidence (real app on Windows 11; shortcuts tested with the simulated buds for determinism, then with the
real Buds 4; Notepad as "another app in front"):

- Shortcuts: `tauri-plugin-global-shortcut`; combinations stored in `settings.shortcuts` as `Ctrl+Alt+KeyN`-style
  strings (KeyboardEvent.code names, parsed by the plugin). A changed shortcut is registered at once; the four
  actions are cycle / anc / transparency / off. Conflicts (another program owns the combination, or it doesn't
  parse) are tracked per action and shown as "Used by another app - pick a different combination." with a red
  outline.
- Recorder (`ShortcutRecorder`): click, press the combination; Esc cancels, Backspace / Delete clears, a combination
  needs a modifier unless it is an F-key (a plain key shakes and is refused), the plugin must accept it. While it
  listens the app's own shortcuts are unregistered (verified: pressing the OLD combination while recording just
  records it; no mode change, no popup). Pure helpers `interpret` / `display` have 7 vitest tests.
- HUD: transparent, topmost, click-through, non-activating window, bottom centre of the monitor under the mouse
  (pure `hud_position`, tested), 44 px icon badge + title + "<Level> level" or device name, fade in 150 ms / hold
  1.4 s / fade out 300 ms, repeated presses restart it; "Buds not connected" variant. Window styles checked with
  Win32: `WS_EX_TOPMOST`, `WS_EX_TRANSPARENT` (click-through), `WS_EX_NOACTIVATE`.
- Verified on screen: with Notepad in front, Ctrl+Alt+N / Ctrl+Alt+T cycle the mode and show the popup at the bottom
  centre; Notepad stayed the foreground window. After a full restart the saved shortcut worked on the very first
  press (real buds). Conflict: a helper process owned Ctrl+Alt+F10 while the app re-registered, and the Off row showed
  the conflict message (other rows unaffected).
- Tests: 3 HUD payload tests, 1 positioning test, 7 recorder helper tests; all suites, clippy and svelte-check clean.

Bugs found and fixed while testing (all would have shipped otherwise):
1. DEADLOCK: recording a shortcut triggered two re-registrations at once (settings-changed task + the
   `set_recording` command on the main thread). The plugin's calls hop through the main thread, so the whole UI froze.
   All (re-)registration now goes through one worker task fed by a channel (requests coalesce, `set_recording` is
   async and waits for completion); the settings-window destroy hook spawns instead of calling directly.
2. Recorder race: the key listener was attached only after the async "pause shortcuts" call returned, so a quick key
   press was missed. It now listens first.
3. Focus theft: creating the HUD webview window on the first shortcut press took the keyboard focus from the app the
   user was typing in. The HUD window is now created (hidden) 2 s after start-up.
4. Untruthful HUD: the controller applies a mode optimistically; the HUD read it before the buds' refusal arrived and
   showed a mode the buds never switched to. The HUD now waits 300 ms for the buds' answer, so a refused change shows
   the real mode.

Left for the owner / noted:
- The buds kept answering mode changes with status 0x0E (refused) during testing (see Phase 7 notes), so a real
  audible switch by shortcut was not confirmed this session; the HUD now reports the refusal honestly. Please retry
  with the buds worn / out of the case.
- Win-key combinations ("Super") are accepted but Windows reserves most of them; the recorder doesn't warn.
- No test on a second monitor or at 150% scaling (positioning maths is unit-tested).
- Testing touched your Notepad window: two stray clicks moved its text cursor (no text changed, 662 characters before
  and after); I minimised nothing and closed nothing. Settings file and registry restored.

## Phase 9: reconnect polish, IPC, diagnostics

Done (hardware-dependent reconnect checks are left for the owner, see docs/TESTING.md). Evidence (Windows 11,
OnePlus Buds 4 connected to this PC):

- **IPC (`budsctl` <-> running app).** `crates/buds-core/src/ipc.rs`: a local socket (named pipe
  `\.\pipe\budscontrol-<USERNAME>`, via `interprocess`), one JSON line request `{"args":[...]}`, answered with
  `{"line":"..."}` lines and `{"exit":N}`. The server runs the very same `cli::execute` on the live controller, so the
  output is identical to a direct run. `budsctl` forwards everything except `devices`, `sdp` and `audio` (which need no
  control connection) unless `--direct`/`--fake`; if nothing listens (800 ms) it talks to the buds itself. The app starts
  the server at start-up (failure is logged, never fatal). 5 integration tests (status, a mode command changes the running
  controller and sends exactly one command, exit codes, 3 clients at once, no app -> `None`) plus 3 unit tests.
  - Verified with the real app (debug build) running: `budsctl status`, `anc`, `transparency` answered by the app
    (exit 0, same block as a direct run, the app's own packet log). `budsctl --direct status` while the app held the
    control connection fails with Winsock 10048 (address in use): the buds allow one connection, which is exactly why
    the IPC exists.
  - Side effects: a second `Start` is ignored by the controller (a late client must not restart the connection);
    `budsctl status` no longer waits 1.2 s when nothing changed.
- **Service discovery (`budsctl sdp`).** `crates/buds-bt/src/win/sdp.rs` uses `WSALookupServiceBegin/Next` (`NS_BTH`).
  Real buds: `HeyMelody 079A: enco link, channel 15`, `1107: none`, `Serial port 1101: RFCOMM COM channel 17, WATCH
  channel 29, enco link channel 15`. Lookup is also used to fill the channel the Developer tab shows when Winsock's
  `getpeername` has none (a connection logs `connected via HeyMelody service 079A (channel 15)`).
- **Audio reconnect.** `crates/buds-bt/src/win/audio.rs`: enumerates render + capture endpoints (all states), follows
  `IDeviceTopology` connector 0 -> `GetDeviceIdConnectedTo`, keeps those whose Bluetooth filter id contains the buds
  address (12 hex digits), then `IKsControl::KsProperty` with `KSPROPSETID_BtAudio` id 0 (one-shot reconnect). It is
  now the first step of `WindowsPlatform::page()`; when it is unavailable the Phase 3 RFCOMM page is used. Diagnostics:
  `budsctl audio` (lists endpoints) and `budsctl audio reconnect`. Real buds: one render endpoint (A2DP sink 110B),
  state active, no capture endpoint (no hands-free profile on this PC); `audio reconnect` accepted (1 audio device),
  exit 0, buds stayed connected.

Bugs found and fixed while testing:
1. A force-killed app left the buds' control service busy for a short time. `budsctl --direct` then got 10048 on 079A,
   the walk went on to the serial port service, "succeeded" on the unrelated `RFCOMM COM` channel 17 (no answer, product
   id `[?]`) **and saved `spp` as the connection hint**, so later connects kept going to the wrong service. Fixes:
   a busy service (10048) now ends the walk (the controller retries later) and the hint is only saved after the buds
   have actually sent something. New unit test for the busy case. The poisoned hint in my settings file was reset to
   `heymelody`; afterwards a direct connect used 079A, channel 15.

Left for the owner / noted:
- Phone-only reconnect (buds playing to the phone, PC asks them to join), the setting being off, and sleep/wake recovery
  need the real situation; they are in docs/TESTING.md (checks 12 and 13).
- I could not confirm that the audio reconnect call by itself pulls the buds over from the phone (the buds were already
  connected here); if it does not, the RFCOMM fallback only runs when the audio call errors, so report it.
- The buds kept refusing mode changes (0x0E) in this session (case battery -1%, probably in the case or idle), so
  `anc` over IPC reached the buds and came back refused; the plumbing is what was verified.

## Phase 10: Windows packaging and release

Done, except the parts that need Windows Sandbox / GitHub (see "Left"). Evidence (Windows 11):

- **Icon.** `crates/icongen` (tiny-skia, no external art) redraws the original icon: blue-to-indigo squircle with glass
  sheen and rim, three-quarter white battery ring, two earbuds. `cargo run -p icongen -- app-icon.png` then
  `npx tauri icon app-icon.png` regenerated `src-tauri/icons/*` (Android/iOS/64 px outputs deleted: not used).
  Looked at the 1024 px render; the earbuds sit slightly lower than the macOS version so the group is centred in the ring.
- **Sidecar.** `scripts/build-sidecar.mjs` builds `budsctl --release` and copies it to
  `src-tauri/binaries/budsctl-<triple>.exe` (gitignored); it runs as part of `beforeBuildCommand`.
  Deviation from the plan: `bundle.externalBin` is in `src-tauri/tauri.bundle.conf.json`, passed with `--config` by the
  packaging script, NOT in `tauri.conf.json`: tauri-build fails when a listed sidecar doesn't exist, which would break
  every plain `cargo build` / `tauri dev` / CI run.
- **Installer (NSIS).** `installMode: currentUser`, WebView2 `downloadBootstrapper`, Start menu folder, uninstaller,
  `src-tauri/nsis/hooks.nsh` (`NSIS_HOOK_POSTUNINSTALL` deletes `HKCU\...\Run\Buds Control`). `mainBinaryName` is
  `BudsControl` so the installed exe is `BudsControl.exe`.
- **Packaging.** `scripts/package-windows.ps1` (`npm ci`, `tauri build`, then installer + portable zip into `release/`;
  the plan said `dist/`, but that is Vite's output folder and is wiped by every frontend build). Output:
  `BudsControl-0.1.0-windows-x64-setup.exe` 1.8 MB, `BudsControl-0.1.0-windows-x64-portable.zip` 2.4 MB (app, budsctl,
  README, LICENSE, notices). Release build takes ~4 min cold.
- **CI.** `.github/workflows/release.yml`: on tag `v*`, a Windows job runs the script and attaches both files to the
  GitHub release (not run yet: needs a pushed tag).
- **README** rewritten: features, install (installer / portable, SmartScreen "More info > Run anyway", tray overflow tip),
  CLI, how it works, development, credits, disclaimer.

Gate 10 checks run on this PC (not in Windows Sandbox):
- Portable zip unpacked to a temp folder: `BudsControl.exe` starts, connects to the real Buds 4, `budsctl.exe` next to it
  answers `status` and `sdp` through the app, transparency / cycle changed the buds' mode. Idle for 60 s: 0.12 s CPU
  (about 0.2 %), 33 MB working set.
- Installer, silent (`/S`): exit 0, no admin prompt, files in `%LOCALAPPDATA%\Buds Control\` (BudsControl.exe, budsctl.exe,
  uninstall.exe), Start menu entry, uninstall entry in HKCU. The installed app started and connected. Uninstalling (`/S`)
  while the app was running closed it and removed the folder, Start menu folder, uninstall entry and the autostart value.
  (The autostart value was written by me with the same name/format the app uses, not by the app's own toggle.)
  My test install was removed again afterwards; settings in `%APPDATA%\BudsControl` were not touched.

Left for the owner / noted:
- Windows Sandbox install check, 5-minute idle CPU, and the full hardware checklist on the release build (docs/TESTING.md).
- The tag-triggered workflow has not run; push a tag `v0.1.0` to try it (see `docs/RELEASING.md`).
- Builds are unsigned (SmartScreen warning); signing needs a certificate.
- `BudsControl.exe` in the zip is the file Tauri patched with the installer type ("nsis"); harmless, only the updater
  looks at it (there is no updater).

## Phase 11: Linux

Code complete, type-checked and linted for Linux. **First run on real Linux: 2026-11-11, see "Phase 11 follow-up" below**
(it was written without any Linux machine: no WSL distribution, no Docker, no remote, so CI hadn't run either). Gate 11 in
docs/TESTING.md has the results; the text of this section is the state before that run.

What was built:
- **Bluetooth backend** (`crates/buds-bt/src/linux`, `bluer` 0.17 with BlueZ over D-Bus). `paired_devices` lists paired
  devices (name or alias, connected, whether the HeyMelody UUID is advertised). `open` uses the shared candidate order:
  for 079A / 1107 / serial port it registers a BlueZ *client profile* for that UUID, calls `Device.ConnectProfile` and takes
  the RFCOMM socket from the `ConnectRequest`; fixed channels 15, 12, 13 use a raw RFCOMM socket. A UUID the buds don't
  advertise counts as "not found", "already connected" counts as busy (so, as on Windows, a busy control service ends the
  walk instead of landing on an unrelated service). One task owns the socket (read + queued writes); dropping the link
  closes it. `page` = `Device.Connect` (BlueZ connects the audio profiles too). `budsctl sdp` prints which services are
  advertised (BlueZ doesn't expose channel numbers).
- **IPC on Unix**: a socket file (0600) in `$XDG_RUNTIME_DIR` (or a private 0700 directory under the temp folder) instead
  of the abstract namespace, which has no access control.
- **App**: tray menu gets "Open Buds Control" first (Linux trays give no click events or icon position); the flyout opens
  at the pointer (`Anchor::Point`, below it or above when there is no room, 2 unit tests) or centred when the pointer
  can't be read (Wayland); solid window and surface (no Acrylic/Mica; the page marks itself `data-platform="linux"` and
  gets a hairline edge; checked in the browser simulator with `?platform=linux`); HUD -> one replaceable desktop
  notification (`notify-rust`, hint transient, same id each time; text function unit-tested); Shortcuts tab shows the
  `budsctl` hint on Wayland (`?platform=wayland`); "Launch at login" text is OS-neutral (the autostart plugin writes
  `~/.config/autostart`). New command `get_platform`.
- **Packaging/CI**: `scripts/package-linux.sh` (AppImage + .deb, deb depends on bluez, libdbus, webkit2gtk-4.1,
  ayatana-appindicator), `release.yml` gets an ubuntu-22.04 job, `ci.yml` gets a whole-workspace clippy + test job on
  Windows and Ubuntu (needs the frontend built first because the app embeds it).

How it was checked without Linux: `rustup target add x86_64-unknown-linux-gnu`, then `cargo clippy --workspace
--all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings` passes (zero warnings). The system
libraries are not available on Windows, so for that check only, `libdbus-sys` was patched with a build script that does
nothing and a tiny stand-in `pkg-config` that claims every library exists (both kept in the scratchpad, not in the
repository). That proves the Linux code compiles and lints; it proves nothing about how BlueZ, the tray or the
notification daemon behave. Windows: 29 + other test suites, clippy, svelte-check, vitest all pass; the debug app's
flyout still opens above the tray (Win32 window rectangle: bottom 812 of a 816 px work area).

Left for the owner / noted:
- Everything that needs Linux: connecting, the profile hand-off, the tray menu, notification, autostart, AppImage / deb
  contents, CI. First thing to try on a Linux box: `budsctl devices`, `budsctl sdp`, `budsctl status`.
- Known risks: BlueZ may refuse `RegisterProfile` without suitable D-Bus policy on some distributions; GNOME needs the
  AppIndicator extension for any tray icon; AppImage autostart paths.
- The GTK flyout loses focus handling differently on Wayland (compositor-dependent), so "closes when you click elsewhere"
  is unverified there.

### Phase 11 follow-up: first run on real hardware (2026-11-11)

Ubuntu 24.04.5 LTS, GNOME 46, Intel laptop, OnePlus Buds 4, driven by Claude over SSH (xdotool and scrot on X11,
gnome-screenshot on Wayland) with the owner at the machine for the parts that need hands. Everything in Phase 11 that
needed Linux was run except the AppImage / .deb and CI; results are in Gate 11 of docs/TESTING.md, the setup notes and
limitations are in [LINUX.md](LINUX.md). The Bluetooth backend worked first time: HeyMelody 079A, RFCOMM channel 15,
connected in about 300 ms.

Bugs found on the hardware and fixed:
- **Reconnect storm.** A link that BlueZ opened but the buds had already dropped made every request of the post-connect
  burst fail, and each failed write queued `Action::Reconnect`, which resets the retry counter and connects at once: about
  25 open/close cycles in 300 ms. `send` now treats a failed write like a closed channel (`link_closed`: close, retry on
  the timer). Regression test `a_link_that_opens_dead_is_retried_on_the_timer_not_in_a_loop` (new fake switch
  `FakePlatform::open_dead`); without the fix that test never finishes.
- **Flyout position (X11).** The flyout was anchored to the pointer, which is on the tray menu, about 250 px above the
  panel. `positioning::panel_strip` now turns the pointer into a spot on the panel strip (the part of the monitor the
  work area leaves out), so the existing icon placement puts the flyout against the panel on any edge. Checked with Dash to
  Panel (bottom), the stock top bar, and the top bar with a fixed left dock.
- **Flyout never shrank (X11).** A non-resizable GTK window keeps the largest size it has had, so collapsing left a tall
  window that the window manager pushed up. The flyout is resizable on Linux (it has no frame). The window manager also
  clamps a move using the old taller size, so the position is applied again 80 ms after each resize.
- **Wayland position.** An app can't place its own window on Wayland (GNOME used the top-left corner). `prefer_xwayland`
  sets `GDK_BACKEND=x11` on Wayland sessions when the user hasn't chosen a backend and the X socket exists (a stale
  `DISPLAY` must not make GTK abort), and the flyout is anchored at the right end of the panel because the pointer is
  unknown. `get_platform` and the anchor code share `app_windows::is_wayland`.

Findings not fixed (also in LINUX.md): the buds refuse a noise-mode change with status `0E` while out of the ears and the
UI says nothing; GNOME shows "Buds Control Settings is ready" because Settings opens without focus; the Wayland limits
(no global shortcuts, flyout at the panel's right end rather than under the icon, Xwayland blur on fractional scaling);
`npm ci` on a lockfile written on Windows lacks the Rolldown Linux binding; a plain `cargo build --release` produces an app
without its web UI (use `npx tauri build --no-bundle`).

Not verified on Wayland: tray menu clicks, the Settings window, the `budsctl` hint text, closing the flyout by clicking
elsewhere, the notification. Not tested at all: AppImage, .deb, "Launch at login", other distributions and desktops.
Checks on both PCs after the changes: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings` and `cargo test --workspace --all-features` pass on Windows and on the Ubuntu laptop.

## Phase 12: macOS

Code complete, type-checked and linted for macOS, **not run on a Mac** (no Mac here, no remote, CI hasn't run). Hardware
items are PENDING (Gate 12 in docs/TESTING.md).

What was built:
- **Bluetooth backend** (`crates/buds-bt/src/macos`, `objc2-io-bluetooth` bindings): a port of the original's connection
  logic. Candidates from the shared walk are mapped to RFCOMM channels: HeyMelody = the channel of the service record named
  "enco" or of UUID 079A; 1107 and the serial port (only reached when the others are "not found") by UUID; fixed
  channels 15, 12, 13 otherwise; a missing HeyMelody record also triggers a fresh SDP query like the original.
  `openRFCOMMChannelAsync` with a delegate (`define_class!`, selectors `rfcommChannelOpenComplete:status:`,
  `rfcommChannelClosed:`, `rfcommChannelData:data:length:`, `connectionComplete:status:`); 3 s per channel, a channel that
  is already open or opens without telling us is used. Paging = `openConnection:withPageTimeout:8000` with a completion
  delegate. All IOBluetooth calls run on the main dispatch queue (`on_main`; worker threads only wait); writes are queued
  on it (a failed write closes the link). Data that arrives before the link exists is buffered.
  Not ported because the controller already covers it: connect / disconnect / channel-open notifications (health tick,
  wall-clock jump after sleep).
- **`budsctl`** runs its command on a worker thread while the main thread runs a run loop (`run_main_loop_until`), because
  IOBluetooth calls back on the main run loop. `budsctl sdp` lists the cached service records. IPC uses the private
  socket file under `$TMPDIR`.
- **App**: Accessory activation policy (no Dock icon) plus `LSUIElement`; template tray icon (the icon is already a single
  colour with transparency, drawn black); flyout with the system Popover material (`macOSPrivateApi` in config and the
  `macos-private-api` feature) and rounded corners; flyout and HUD visible on all Spaces; `Info.plist` with
  `NSBluetoothAlwaysUsageDescription` (the original's text); ad hoc signing; `scripts/package-macos.sh` builds the .dmg for
  the Mac's architecture (no universal build, because the sidecar is built per architecture); release job on
  `macos-latest` (arm64) and `macos-13` (x64); the CI whole-workspace job now includes `macos-latest`.

How it was checked without a Mac: `rustup target add aarch64-apple-darwin`, then `cargo clippy --workspace --all-targets
--all-features --target aarch64-apple-darwin -- -D warnings` passes with no warnings. Only one build script needs a C
compiler (`objc2-exception-helper`); for this check only it was patched with an empty build script (kept in the
scratchpad, not in the repository). The type checker found one real bug on the way (edition-2024 closures captured the
non-`Send` field instead of the `Send` wrapper). That proves the code compiles and is thread-safe in the Rust sense; it
proves nothing about how IOBluetooth, the delegate callbacks, the run loop, the tray or the vibrancy behave. Windows and Linux
checks were re-run after the changes.

Left for the owner / noted:
- Everything that needs a Mac. First things to try: `budsctl devices`, `budsctl sdp`, `budsctl status`.
- Likely trouble spots: the delegate selectors / argument encodings (a mismatch would show as "channel opened but no
  data"), the Bluetooth permission prompt for an ad hoc signed app, `NSPort` keeping the budsctl run loop alive, Popover
  vibrancy with a transparent window, and the template icon's size at 1x / 2x.
- Builds are not notarised (Gatekeeper prompt). The original Swift app remains the tested choice on macOS.
