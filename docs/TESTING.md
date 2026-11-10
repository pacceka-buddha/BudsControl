# Hardware testing

Real-hardware checks for Buds Control, run by the project owner with **OnePlus Buds 4** (product ID `065414`).
Automated tests (`cargo test --workspace`) cover everything that doesn't need the buds; this list covers the rest.
Fill in the **Result** column (pass / fail / note) and the date; re-run at Gates 3, 5, 9 and 10 (see `PLAN.md`).

## Gate 3: `budsctl` against the real buds (Phase 3)

Build once: `cargo build -p budsctl` -> `target\debug\budsctl.exe` (use the full path or add it to `PATH`).
Pair the buds in **Windows Settings > Bluetooth & devices**, take them out of the case, and make sure they are
**connected to this PC** (they show "Connected" and the PC is the audio output). Then:

| # | Command | Expected | Result |
|---|---|---|---|
| 3.1 | `budsctl devices` | the Buds 4 listed as `connected`, `buds: yes` |  pass 2026-11-10 (Buds 4 `connected`, `buds: yes`) |
| 3.2 | `budsctl status` | `device:  OnePlus Buds 4 [065414] connected`, real L/R/case battery, current mode, EQ, low latency, gestures; exit code 0 |  pass 2026-11-10 (`[065414]`, L 100% R 100%, mode, EQ, gestures read; exit 0) |
| 3.3 | `budsctl anc`, `budsctl transparency`, `budsctl off` | the mode audibly changes each time; `status` agrees | |
| 3.4 | `budsctl smart`, `max`, `moderate`, `mild`, `adaptive` | each ANC level applies and reads back (`mode:    anc (<level>) [bit N]`) | |
| 3.5 | `budsctl watch 30`, meanwhile take a bud out / put it back / use a gesture / long-press for a mode | pushes show up as `<- AA ... 04 02 ...` lines; `HOSTS` line lists the paired devices | |
| 3.6 | `budsctl raw 0106` | a `-> AA 07 00 00 06 01 ..` line, then a `<- ..` line whose command bytes are `06 81` |  pass 2026-11-10 (reply `06 81`) |
| 3.7 | `budsctl eq=1`, `budsctl eq=0` | EQ preset changes; `status` shows the preset name | |
| 3.8 | `budsctl custom=0,2,4,2,0,-2` (needs a custom EQ on the buds) | band gains applied; `status` lists `62Hz:0 ...` | |
| 3.9 | `budsctl lowlatency=on` then `off` | `status` shows `lowlat:  on` / `off` | |
| 3.10 | `budsctl find=on` (**loud: take the buds out of your ears first**), then `find=off` | sound plays and stops | |
| 3.11 | `budsctl gesture=left,doubleTap,next`, then a real double tap on the left bud | the tap now skips to the next track; set it back afterwards | |
| 3.12 | Put the buds in the case, run `budsctl status` | `searching`, exit code 1, no hang (about 8 s) | |

**Record in `docs/PROGRESS.md`** which candidate connected. `budsctl status` prints the packet log; the lines to look
for are `trying ...` and `connected via ... (channel N)`. Expected: HeyMelody service 079A or 1107, or channel 15.
The same text is in `%LOCALAPPDATA%\BudsControl\BudsControl.log`. **If no candidate connects, stop and report**
(include that log).

## Full checklist (re-run at Gates 5, 9, 10)

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

## Without hardware

`budsctl --fake <command>` (or `BUDS_FAKE=1` for the app, from Phase 4) runs against a simulated Buds 4, so
commands and UI can be tried anywhere.
