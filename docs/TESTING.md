# Hardware test log

How Buds Control was checked on real earbuds, and what has not been checked yet. The automated tests
(`cargo test --workspace`, `npm test`) cover everything that does not need earbuds; this log covers the rest.

Checks are grouped in numbered **gates**. Each row says what to do, what should happen and, once it has been run, the
result with its date. An empty result means the row has not been run. Longer explanations are in the notes under each
table.

| Gate | Covers | State |
|---|---|---|
| 3 | `budsctl` against the real buds, on Windows (OnePlus Buds 4) | Run 2026-11-10 and 2026-11-18, every row passes |
| Full checklist | The whole app, on Windows | Results not recorded row by row |
| 9 | Reconnecting from a phone-only connection, sleep and wake | Results not recorded |
| 10 | The Windows release build | Results not recorded |
| 11 | Linux (Ubuntu 24.04, GNOME, Buds 4) | Run 2026-11-11, 2026-11-16 and 2026-11-18, partly done |
| 12 | macOS (macOS 27, Apple silicon, Buds 3) | Run 2026-11-16 and 2026-11-18, partly done |

Intel Macs and Linux desktops other than GNOME are untested.

## Gate 3: `budsctl` against the real buds

Build once: `cargo build -p budsctl` -> `target\debug\budsctl.exe` (use the full path or add it to `PATH`).
Pair the buds in **Windows Settings > Bluetooth & devices**, take them out of the case, and make sure they are
**connected to this PC** (they show "Connected" and the PC is the audio output). Then:

| # | Command | Expected | Result |
|---|---|---|---|
| 3.1 | `budsctl devices` | the Buds 4 listed as `connected`, `buds: yes` | pass 2026-11-10 |
| 3.2 | `budsctl status` | `device:  OnePlus Buds 4 [065414] connected`, real L/R/case battery, current mode, EQ, low latency, gestures; exit code 0 | pass 2026-11-10 |
| 3.3 | `budsctl anc`, `budsctl transparency`, `budsctl off` | the mode audibly changes each time; `status` agrees | pass 2026-11-10 (read back); heard 2026-11-18 |
| 3.4 | `budsctl smart`, `max`, `moderate`, `mild`, `adaptive` | each ANC level applies and reads back (`mode:    anc (<level>) [bit N]`) | pass 2026-11-10: smart bit 7, max 4, moderate 5, mild 6, adaptive 11 |
| 3.5 | `budsctl watch 30`, meanwhile take a bud out / put it back / use a gesture / long-press for a mode | pushes show up as `<- AA ... 04 02 ...` lines; `HOSTS` line lists the paired devices | pass 2026-11-18 (in the app) |
| 3.6 | `budsctl raw 0106` | a `-> AA 07 00 00 06 01 ..` line, then a `<- ..` line whose command bytes are `06 81` | pass 2026-11-10 |
| 3.7 | `budsctl eq=1`, `budsctl eq=0` | EQ preset changes; `status` shows the preset name | pass 2026-11-10: Vocal, Bass, Balanced |
| 3.8 | `budsctl custom=0,2,4,2,0,-2` (needs a custom EQ on the buds) | band gains applied; `status` lists `62Hz:0 ...` | pass 2026-11-10: Custom1 set, then reset to 0 |
| 3.9 | `budsctl lowlatency=on` then `off` | `status` shows `lowlat:  on` / `off` | pass 2026-11-10 |
| 3.10 | `budsctl find=on` (**loud: take the buds out of your ears first**), then `find=off` | sound plays and stops | pass 2026-11-18 (in the app) |
| 3.11 | `budsctl gesture=left,doubleTap,next`, then a real double tap on the left bud | the tap now skips to the next track; set it back afterwards | pass 2026-11-18 (in the app) |
| 3.12 | Put the buds in the case, run `budsctl status` | `searching`, exit code 1, no hang (about 8 s) | pass 2026-11-18 |
| 3.13 | Buds out of the case, then open the app from the Start menu | battery and controls appear | pass 2026-11-18, after a fix (note below) |

Notes:

- **2026-11-10:** each command ran in a fresh session and the result was read back from the buds. The audible change in
  3.3 was not checked that day.
- **3.5, 3.10, 3.11 (2026-11-18):** done by hand through the app instead of `budsctl watch`, `find=on` and `gesture=`. For
  3.11 the left double tap was set to play/pause, a real double tap paused and resumed music, and the setting was
  restored afterwards.
- **3.12:** with the buds in the case and the app closed, `budsctl` paged the buds, retried after 1, 2, 3 and 5 s, and
  printed `searching` with an empty state and exit code 1.
- **3.13, the connection-hint bug:** after the buds came out of the case, the app showed "Connected" with no battery
  and no controls. Right after the buds reconnect, the control service can time out. The app then fell back to the serial
  port service, which accepts the connection but sends data that are not protocol frames. Because *any* received bytes
  counted as an answer, the serial port was saved as the connection hint, and every later start used it first and never
  reached 079A. Now a hint is saved only after a well-formed frame arrives, and it is dropped when the buds stop
  answering. Regression test: `a_remembered_service_that_goes_silent_is_forgotten_until_it_answers_again`. Checked by
  setting the hint to `spp` by hand (the app recovered in under 4 s) and by a real case test (HeyMelody 079A tried first,
  connected in 1.8 s).

Note which candidate connected. `budsctl status` prints the packet log; the lines to look for are `trying ...` and
`connected via ... (channel N)`. Expected: HeyMelody service 079A or 1107, or channel 15. The same text is in
`%LOCALAPPDATA%\BudsControl\BudsControl.log`. If no candidate connects, stop and report it, with that log.

## Full checklist

| # | Check | Result |
|---|---|---|
| 1 | Paired-but-disconnected buds show their name and "Not connected"; Retry works | |
| 2 | Take buds out of the case: Connected within ~6-10 s; L/R/Case battery shown; mode matches the buds | |
| 3 | Off / Transparency / ANC switch audibly; UI changes instantly and doesn't flick back | |
| 4 | ANC levels Smart, Max, Moderate, Mild, Adaptive each apply and read back | |
| 5 | Mode changed on the bud (long press) shows in the UI within ~1 s | |
| 6 | Battery and charging bolt update when a bud goes into the case | |
| 7 | EQ presets (Balanced / Vocal / Bass) and custom EQ bands apply and persist after reopening | |
| 8 | Low latency toggle applies; a gesture-triggered change shows up | |
| 9 | Find earbuds plays, Stop stops, auto-stops after 30 s | |
| 10 | Gestures: change Left double tap to Next track and verify with a real tap; Right side too | |
| 11 | Buds into the case: "Not connected" (no stuck "Connecting..."); out again: reconnects without clicking | |
| 12 | Auto-reconnect from a phone-only connection (Gate 9); disabled when the setting is off | |
| 13 | Sleep / wake recovers | |
| 14 | Shortcuts + HUD + conflict detection | |
| 15 | Remember settings: pick ANC Max, change to Transparency in the phone app, reconnect to the PC: Max is restored | |
| 16 | Launch at login: tray icon present, no flyout | |
| 17 | Developer tab: packet log, raw `0106`, scan, noise bits, multipoint hosts | |
| 18 | `budsctl` direct (app closed) and via IPC (app running) | |
| 19 | Tray icon ring / dim / theme; tooltip | |
| 20 | DPI 100 / 150 / 200 %, two monitors, taskbar at top / left: flyout and HUD placement | |

## Gate 9: reconnect (needs the buds in a real phone and computer situation)

1. Connect the buds to your phone only (disconnect them from the PC in Windows). Open Buds Control: it should show
   "Connecting..." / "Searching..." and, within ~10 s, Connected (the audio reconnect asks the buds to join the PC).
   Check the Developer tab log for `audio reconnect requested`. If it says `audio reconnect not used: ...`, put that
   line and the output of `budsctl audio` in your report.
2. Turn "Reconnect buds to this PC automatically" off in Settings and repeat: the app must NOT pull the buds over by
   itself; Retry in the flyout must.
3. Sleep the PC for about a minute and wake it: the buds reconnect without clicking anything.
4. With the app running, from a terminal: `budsctl status`, `budsctl cycle` (answered by the app, flyout updates), and
   `budsctl --direct status` (fails with "address in use" while the app is running: expected).
5. `budsctl sdp` shows `HeyMelody 079A: ..., channel 15`; `budsctl audio` lists the render endpoint.

## Gate 10: release build

1. `./scripts/package-windows.ps1`, or download the artifacts of a tagged release.
2. Windows Sandbox (no Bluetooth): run the setup exe: it installs without an admin prompt, the app starts in the tray
   (overflow area), the uninstaller (Settings > Apps) removes it; turn on "Launch at login" first and confirm
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` no longer has "Buds Control" afterwards.
3. Real PC: the portable zip runs from any folder and `budsctl.exe` next to it works; run the whole checklist above on
   the release build; leave it idle for 5 minutes (Task Manager: about 0 % CPU).
4. Tag `v1.0.0` and confirm the Release workflow attaches the setup exe and the portable zip.

## Gate 11: Linux (Ubuntu 24.04.5, GNOME 46; runs 2026-11-11, 2026-11-16 and 2026-11-18; partly done)

Needs a Linux machine with BlueZ and OnePlus / OPPO / realme buds, X11 and Wayland sessions if possible. Setup notes,
known issues and the Wayland limitations are in [LINUX.md](LINUX.md).

Run over SSH with someone at the laptop for the parts that need hands (buds in and out of the case or ears, looking at
the screen). The first runs used a binary built with `npx tauri build --no-bundle`; the packages were tested on
2026-11-18 (row 11.15). Buds: OnePlus Buds 4, product ID `065414`.

| # | Check | Result |
|---|---|---|
| 11.1 | `budsctl devices` lists the paired buds; `budsctl sdp` shows `HeyMelody 079A: advertised` | pass: `connected`, `buds: yes`; 079A advertised, 1107 and serial port not advertised |
| 11.2 | `budsctl status` (app closed) connects and prints the state block | pass: connected via HeyMelody 079A, channel 15, in about 300 ms |
| 11.3 | Modes and ANC levels, EQ, low latency through the running app (`budsctl` hands over to it) | pass, read back from the buds; mode changes are rejected with `0E` while the buds are not in the ears (see LINUX.md) |
| 11.4 | Gestures: change three, read back, restore | pass |
| 11.5 | Find earbuds start and stop | pass: both acknowledged, sound heard to start and stop |
| 11.6 | Tray menu, flyout, Settings window (X11) | pass, after two fixes (note below) |
| 11.7 | Flyout placement under Dash to Panel, the stock top bar, and the top bar with a left dock | pass: 12 px above a bottom panel, 12 px below a top bar; stays put when it expands and collapses |
| 11.8 | Shortcuts on X11: record all four, trigger each from another window | pass: Ctrl+Alt+N / J / O set ANC / Transparency / Off, Ctrl+Alt+B cycled all three; the popup is a desktop notification that is replaced, not stacked |
| 11.9 | Tray "Reconnect" | pass: reconnected in about 190 ms |
| 11.10 | Real disconnect: buds into the case, wait, out again | pass after a fix (note below); back in about 15 s |
| 11.11 | Wayland session: app starts, connects, second launch opens the flyout, flyout position | pass with the Xwayland change: flyout at the right end of the panel |
| 11.12 | Wayland: global shortcut | does not fire (expected); a synthetic key press was used, so not conclusive |
| 11.13 | Wayland: tray menu clicks, Settings window, "use `budsctl`" hint, closing the flyout by clicking elsewhere, shortcut notification | **not checked** |
| 11.14 | The Release checklist rows (1-8, 11, 14, 16-18) by number | **not mapped**: the features were exercised, but not against that numbered list |
| 11.15 | AppImage and .deb installs | pass 2026-11-18 (note below) |
| 11.16 | "Launch at login" | **not tested** (an autostart entry did get created during testing; it is not known whether by a click or by the test script) |

Notes:

- **11.6:** the flyout first sat far above the panel and did not return to the panel edge after collapsing (X11 window
  sizing). Both are fixed.
- **11.10:** before the fix, the app opened and closed the link about 25 times in 300 ms, because each failed write
  reconnected at once. Now there is one failed write, one timed retry, and the buds are back in about 15 s. A regression
  test covers it.
- **11.11:** without the Xwayland change GNOME put the flyout in the top-left corner.
- **11.15 (2026-11-18, GNOME on X11, Buds 4, built from commit `7f75bd7`):**
  - `sudo apt install` of the .deb finished cleanly. The only message was apt noting that it read the file from a home
    folder as root. It installed `/usr/bin/BudsControl`, `/usr/bin/budsctl` and the "Buds Control" menu entry.
  - The installed app connected on RFCOMM channel 15. The first write hit a dead link and the app retried after 1 s.
    Modes, ANC levels, EQ, low latency and find earbuds worked, and the flyout sat just above the bottom panel.
  - The AppImage (88 MiB) mounted, started and connected, and the state was read through its bundled `budsctl`.
  - Build problems: with Node 22.11, `npm ci` skipped the bundler's Linux native binary because it needs a newer Node
    (`npm install --no-save @rolldown/binding-linux-x64-gnu@1.2.18` worked around it), and the AppImage step timed out
    downloading `linuxdeploy` until the file was placed in `~/.cache/tauri` by hand.

Re-run 2026-11-16 (same laptop, GNOME on X11, Buds 4), built from the working tree with the macOS and `plan.rs` fixes:
`fmt`, `clippy -D warnings` and the whole workspace test suite pass on Linux; `devices`, `sdp`, `status` (connected via
channel 15 in 0.24 s), all modes and ANC levels including adaptive, EQ, low latency and a gesture change read back from
the buds; `budsctl` hands over to the running app (7 ms) and `--direct` is refused ("busy"); reconnect through the app
took 1.2 s; the flyout sits 12 px above the bottom panel; the idle app uses about 0.5 % CPU. Not repeated: find
earbuds, shortcuts, tray menu clicks, buds into the case, Wayland.

If the connection fails, include the log (`~/.local/share/BudsControl/BudsControl.log`) and `budsctl sdp` in your
report.

## Gate 12: macOS (runs 2026-11-16 and 2026-11-18 on macOS 27.0.1, Apple silicon, OnePlus Buds 3 `063C14`; partly done)

Needs a Mac (11+) with OnePlus / OPPO / realme buds. The original Swift app is the reference for how it should behave.

Run over SSH (Tailscale) with someone at the Mac for the parts that need hands, and built on the Mac from source.
Two macOS details shape how this is run:

- `./scripts/package-macos.sh` fails at the .dmg step over SSH, because that step needs a desktop session. Started from a
  script opened in Terminal with `open -a Terminal x.command` (output written to a file), it builds the .dmg too; that is
  how the 2026-11-18 .dmg was made.
- Over SSH macOS gives `budsctl` no Bluetooth permission. Terminal.app has to run it once and be allowed, so the
  same `open -a Terminal` trick is used for `budsctl`.

| # | Check | Result |
|---|---|---|
| 12.1 | `budsctl devices`, `sdp` | pass: Buds 3 `connected`, `buds: yes`; `sdp`: enco link channel 15, serial port channel 12 |
| 12.2 | `budsctl status` (app closed) | pass: HeyMelody 079A, channel 15; L/R/case battery, mode, EQ, low latency, gestures read |
| 12.3 | Modes and ANC levels (off, transparency, anc, smart, max, moderate, mild) | pass, read back, with the buds **in the ears**; out of the ears they are rejected with `0E` (as on Linux). `adaptive` is not offered by the Buds 3 |
| 12.4 | EQ presets, low latency on/off | pass, read back from the buds |
| 12.5 | Gestures: change two, read back, restore | pass |
| 12.6 | Find earbuds from the app | pass (heard) |
| 12.7 | `budsctl` with the app running (handover) | pass after a fix (note below) |
| 12.8 | `budsctl find=on` on its own (no app) | no sound heard on 2026-11-16; passes through the app on 2026-11-18 (note below) |
| 12.9 | First connect speed | **open** (note below) |
| 12.10 | .dmg: Gatekeeper warning, Bluetooth prompt | pass 2026-11-18; Bluetooth prompt **not reported** (note below) |
| 12.11 | Menu bar icon, flyout, Settings, Developer tab | pass 2026-11-18 |
| 12.12 | Release checklist rows (1-8, 11-14, 16-18) by number | **not mapped**; Intel Mac **untested** (no hardware, will not be tested here) |
| 12.13 | Spaces, full-screen apps, shortcut popup (HUD), Cmd shortcuts | **not checked** |
| 12.14 | Out of the case, then open the app right away (the connection-hint bug of 3.13) | pass 2026-11-18: HeyMelody 079A tried first, connected in 0.12 s, no serial port fallback |

Notes:

- **12.7:** the app's command socket path was longer than macOS's 104-byte limit, so it was never created. A short
  fallback path fixed it. Now `budsctl status` is answered by the app (0 s) and `--direct` collides with it, as on the
  other platforms.
- **12.8:** `budsctl find=on` on its own made no sound, though the command succeeded; the buds probably stop when the
  link closes right after. On 2026-11-18 the app's Find earbuds button, with the buds out of the ears, played and
  stopped on click. So the silent `budsctl` case is most likely the link closing, not a protocol fault.
- **12.9:** on 2026-11-16 the first RFCOMM open in a fresh `budsctl` process often hung for the 3 s timeout, and the retry
  connected in about 0.2 s. A fast first open followed an app quit; holding the device object and a longer wait before
  exit did not help. The cause was not found. On 2026-11-18 it was not seen in the app (connected in 0.12 s after a
  start, 0.3 s on the first connection); the `budsctl` case was not re-run.
- **12.10:** the .dmg (`BudsControl-0.1.0-macos-arm64.dmg`, 3.3 MB) was built on the Mac, copied to Downloads with the
  quarantine flag set and opened. The window shows the app icon and an Applications shortcut, and the drag to
  Applications worked. On first launch Gatekeeper says "Buds Control" Not Opened, "Apple could not verify it is free of
  malware", with **Done** and **Move to Bin** only (no Open button). **Open Anyway** in System Settings > Privacy &
  Security then started it, and it connected to the Buds 3 with the full flyout. Whether a Bluetooth permission prompt
  appeared was not reported; the permission could not be reset over SSH, so it may have been granted already.
- **12.11:** the menu bar icon, the flyout under it (modes, levels, EQ, low latency, find earbuds, gestures), the Settings
  window with its General, Shortcuts and Developer tabs, and the Developer log, which fills with the traffic, including
  the buds' pushes and the `0E` rejections while the buds are out of the ears.

What to check on a Mac, if you are testing one:

1. Open the .dmg build: Gatekeeper warning (ad hoc signature), then the Bluetooth permission prompt appears once.
2. No Dock icon; a template icon in the menu bar that follows light/dark menu bars; click opens the flyout under it with
   the vibrancy material, clicking elsewhere closes it; works on another Space and over a full-screen app.
3. `budsctl devices`, `budsctl sdp` (expect an "enco link" record with channel 15 on the Buds 4), `budsctl status`
   (app closed), then the checklist above: at least rows 1-8, 11, 12, 13, 14, 16, 17, 18.
4. Shortcut popup (HUD) shows on every Space; Cmd-based shortcuts record correctly.
5. If the connection fails, include `~/Library/Logs/BudsControl.log` and `budsctl sdp` in your report.

## Without hardware

`budsctl --fake <command>` (or `BUDS_FAKE=1` for the app) runs against a simulated Buds 4, so commands and UI can be
tried anywhere.
