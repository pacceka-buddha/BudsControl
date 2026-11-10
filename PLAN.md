# Buds Control for Windows (+ Linux, macOS): Rust + Tauri 2 port plan

## Context

[amarsakore/BudsControl](https://github.com/amarsakore/BudsControl) (MIT, Swift/SwiftUI, ~4.2k lines) is a macOS
menu-bar app that controls OnePlus / OPPO / realme earbuds over the HeyMelody RFCOMM protocol: noise mode and ANC
levels, battery, EQ presets and custom EQ, low latency, find my earbuds, gestures, global shortcuts with a HUD,
automatic reconnect, remembered settings, developer tools (packet log, raw commands, noise bits, multipoint
list) and a CLI. We are building a **Windows port that sits in the taskbar notification area (tray)** with the same
features. The user also wants **Linux and macOS** support, has **OnePlus Buds 4** for hardware testing, and wants
a **portable exe + installer**.

Locked decisions (from the project owner):
- **Rust + Tauri 2** (Svelte 5 + TypeScript frontend in the system WebView).
- **Windows first and complete**; Linux and macOS backends are later phases in this same plan.
- Ship Windows as **portable zip + NSIS installer**.
- Hardware checkpoints use **OnePlus Buds 4** (product ID `065414`).

State of the machine (checked): `D:\nilcodes\BudsControl` is empty and not a git repo. Installed: Node 24.19,
npm 11.17, git 2.55, Python 3.12/3.15, winget 1.29, WebView2 157. **Missing: Rust toolchain and MSVC Build Tools.**
Upstream reference was read at commit `298d4527337b86052a7ebac8f1c2f52df451239f`.

---

## 1. Architecture

```
┌──────────────────────── Tauri app (src-tauri, crate "budscontrol") ────────────────────────┐
│ tray.rs  windows.rs(flyout/settings/hud)  commands.rs  hotkeys.rs  hud.rs  ipc.rs  theme.rs │
│        ▲ invoke / emit (JSON)                                   ▲ ControllerHandle          │
│  Svelte UI: flyout.html · settings.html · hud.html              │                           │
└─────────────────────────────────────────────────────────────────┼───────────────────────────┘
        budsctl (CLI) ──── local socket IPC (if app running) ─────┤ or its own controller
                                                                  ▼
┌──────────── crates/buds-core (pure Rust, no OS code) ────────────┐   ┌── crates/buds-bt ──────┐
│ protocol (Frame, FrameParser) · parse/encode · types · catalog   │   │ Platform impls:        │
│ controller (actor: state machine, timers, snapshot) · settings   │◄──┤  windows (Winsock      │
│ packet_log · cli (shared command executor) · Platform trait      │   │   AF_BTH + BluetoothAPIs)│
│ fake (FakePlatform + simulated Buds 4, feature "fake")           │   │  linux (bluer/BlueZ)   │
└──────────────────────────────────────────────────────────────────┘   │  macos (objc2 IOBluetooth)│
                                                                       └────────────────────────┘
crates/buds-art: tiny-skia drawing of the earbuds glyph + battery ring (tray icon and app icon)
```

Key principles:
1. **All behaviour lives in `buds-core`** (ported line-by-line from `BudsController.swift`). UI and CLI only
   render a `Snapshot` and send `Action`s. Derived values (available modes, `supports()`, EQ choices, gesture
   options) are computed in Rust, never re-implemented in TypeScript.
2. **Controller = single-threaded actor** (the Swift code runs everything on the main thread; we run everything
   on one tokio task). Every platform callback, timer and user action becomes a `Msg` in one `mpsc` inbox.
   State is published through a `tokio::sync::watch<Snapshot>`.
3. **OS-specific code only behind the `Platform` trait** (Bluetooth) and small per-OS modules in the app
   (theme/accent colour, tray icon size, flyout anchoring, Windows audio reconnect).
4. **Fake mode everywhere**: `BUDS_FAKE=1` (app) / `budsctl --fake` runs against a simulated Buds 4, so the whole UI
   can be built and checked without hardware and on CI.

### Repository layout (root = `D:\nilcodes\BudsControl`)

```
Cargo.toml                 [workspace] members = ["crates/*", "src-tauri"]; resolver = "2"
package.json  vite.config.ts  svelte.config.js  tsconfig.json
flyout.html  settings.html  hud.html            Vite multi-page entries
src/                       Svelte frontend
  flyout/   main.ts Flyout.svelte Header.svelte ModeControls.svelte BatteryRow.svelte MoreControls.svelte Disconnected.svelte
  settings/ main.ts Settings.svelte GeneralTab.svelte ShortcutsTab.svelte DeveloperTab.svelte PacketLog.svelte
  hud/      main.ts Hud.svelte
  lib/      api.ts stores.ts format.ts platform.ts bindings/ (ts-rs output)
  lib/components/ Segmented.svelte Card.svelte Row.svelte ToggleSwitch.svelte PickerButton.svelte Button.svelte
                  BatteryRing.svelte EarbudsGlyph.svelte Icon.svelte EqBands.svelte BandSlider.svelte
                  ShortcutRecorder.svelte Spinner.svelte
  lib/styles/ tokens.css base.css
src-tauri/                 Tauri app crate (Cargo.toml, build.rs, tauri.conf.json, capabilities/, icons/, Info.plist)
  src/ main.rs lib.rs state.rs tray.rs tray_render.rs windows.rs positioning.rs commands.rs hotkeys.rs hud.rs ipc.rs
       theme.rs audio_reconnect_win.rs
crates/buds-core/src/      lib.rs types.rs protocol.rs parse.rs encode.rs catalog/{mod.rs,generated.rs}
                           controller/{mod.rs,state.rs,snapshot.rs,timers.rs,connect.rs,frames.rs,remember.rs}
                           platform.rs settings.rs packet_log.rs cli.rs ipc_proto.rs fake.rs
crates/buds-bt/src/        lib.rs windows/{mod.rs,devices.rs,rfcomm.rs,sdp.rs} linux/mod.rs macos/mod.rs
crates/buds-art/src/       lib.rs (glyph + ring drawing on tiny-skia)
crates/budsctl/src/        main.rs
crates/icongen/src/        main.rs (renders app-icon.png 1024px)
tools/convert_catalog.py   upstream DeviceCatalog.swift -> crates/buds-core/src/catalog/generated.rs
reference/upstream/        git clone of the original (gitignored), pinned to 298d452
docs/ PROTOCOL.md ARCHITECTURE.md TESTING.md (hardware checklist)
scripts/ package-windows.ps1 build-sidecar.mjs
.github/workflows/ ci.yml release.yml
LICENSE README.md THIRD_PARTY_NOTICES.md .gitignore
```

### Porting map (upstream file → new home)

| Upstream (Swift) | New location | Notes |
|---|---|---|
| `Protocol.swift` | `buds-core/types.rs`, `protocol.rs` | Commands, Frame, FrameParser (LEB128), enums, EQ names, gestures |
| `Devices.swift` | `buds-core/catalog/mod.rs` | `DeviceModel`, `DeviceFeature` + capability bits, generic/realme models, lookup |
| `DeviceCatalog.swift` | `catalog/generated.rs` via `tools/convert_catalog.py` | 105 models, never hand-edited |
| `BudsController.swift` | `buds-core/controller/*`, `parse.rs`, `encode.rs`; connection part → `buds-bt/*` | The heart; port every branch |
| `AppModel.swift` | `src-tauri/state.rs`, `hotkeys.rs`; `buds-core/packet_log.rs` | perform(action) + HUD, recording mode, log ring + file |
| `main.swift` (CLI modes) | `buds-core/cli.rs` + `crates/budsctl` | `--cli`, `--watch`, `--scan`, `--raw`, `--sdp` |
| `MenuView.swift`, `MoreControls.swift`, `LiquidSegmented.swift`, `BatteryRing.swift` | `src/flyout/*`, `src/lib/components/*` | Windows 11 Fluent look, same structure |
| `SettingsView.swift` | `src/settings/*` | General / Shortcuts / Developer |
| `HotKeys.swift` | `src-tauri/hotkeys.rs` + `ShortcutRecorder.svelte` | `tauri-plugin-global-shortcut` |
| `HUD.swift` | `src-tauri/hud.rs` + `src/hud/*` | Click-through topmost pill |
| `BatteryRing.swift` `MenuBarIcon` | `buds-art` + `src-tauri/tray_render.rs` | Dynamic tray icon |
| `tools/MakeIcon.swift` | `crates/icongen` | Same design, drawn with tiny-skia |
| `GlassCompat.swift` | — | Replaced by Tauri window effects (Mica/Acrylic/vibrancy) |

---

## 2. Core design details (the implementer must follow these)

### 2.1 Protocol (port exactly; also write `docs/PROTOCOL.md` from upstream README + parser comments)

Frame: `AA <len> 00 00 <cmd lo> <cmd hi> <seq> <plen lo> <plen hi> <payload…>`; encode writes `len = payload+7`
as **one byte** (`& 0xFF`, as upstream); the parser reads `len` as **LEB128** (max shift 21). Replies = command |
0x8000. Seq is a wrapping `u8` incremented before each send.

| Cmd | Name | Sent payload | Reply/push parse (`p[0]` = status) |
|---|---|---|---|
| 0x0100 | hello | – | capabilities: bits of `p[1..]`, LSB first, bit index = byteIndex*8+bit; only if `p[0]==0` |
| 0x0103 | productID | – | if `p.len()>=4 && p[0]==0`: pid = `format!("{:02X}{:02X}{:02X}", p[3],p[2],p[1])` |
| 0x0106 | battery | – | `<st> <n> (<id> <lvl\|0x80 charging>)*`; 1 L, 2 R, 3 case; case = None if level 0 and not charging |
| 0x0109 | wear | – | `<st> <n> (<side> <0 case\|1 out\|3 in>)*`, merged into existing map |
| 0x010C | noise query | `01 01` | find `01 01 <lo> [<hi>]`; mask≠0; bit = trailing zeros; type = `reportedBits[bit]` else log "unknown noise bit" |
| 0x0404 | set noise | `01 01 <mask bytes LE, count = bit/8+1>` | ack: log if `p[0]!=0`, then send a noise query |
| 0x0403 | set switch | `<feature id> <0/1>` (low latency id = 6) | if `p[0]!=0`: log + re-query `0x010D [01 06]`; else do nothing (push confirms) |
| 0x010D | switch query | `01 06` | `<st> <n> (<id> <0/1>)*` → gameMode; then `restore_settings()` |
| 0x010F / 0x0406 | EQ query / set EQ | – / `<id>` | query: `p[0]==0 && len>=2` → eqID=`p[1]`; set ack → re-send `0x010F` and `0x0122 [01 05]` |
| 0x0122 / 0x0418 | custom EQ list / set | `01 05` / see encode | list: `<st> <n> (<sel> <min i8> <max i8> <id> <nameLen> <name> <bands> (<hz lo> <hz hi> <gain i8>)*)*`; empty name → "Custom"; only assign if different |
| 0x0108 / 0x0401 | gestures query / set | – / `01 <side> 01 <gesture> <action>` | `<st> <n> (<side> <ctx> <gesture> <action>)*`, keep ctx==01 and known enums only; set ack → re-query |
| 0x0400 | find buds | `01` start / `00` stop | if finding && `p[0]!=0` → log rejected, finding=false |
| 0x0112 | multipoint | – | `<st> <n> (<addr 6 bytes reversed> <rsv> <2=connected> <1=this host> <nameLen> <name>)*` |
| 0x040B | multipoint set | – | log if rejected |
| 0x0205 | subscribe | `03 01 02 03` | – |
| 0x0204 | push | – | `p[0]`: 1 battery, 2 wear, 3 noise, 5 → gameMode=`p[1]!=0` (len≥2), 6 hosts, other → `refresh()` |

Custom EQ encode (`setCustomEQ`): `02 <min i8> <max i8> <id> <nameLen> <name utf8 ≤255> <bandCount> (<hz lo> <hz hi> <gain i8>)*`,
gains clamped to `[min,max]`. ModeType/ANCLevel/EQ-name/GestureAction tables: copy from `Protocol.swift:133-308`.

### 2.2 Catalog
`DeviceModel { name: &'static str, brand: Brand, mode_types: &'static [u8], set_bits: &'static [(u8,u8)],
reported_bits: &'static [(u8,u8)], eq_presets: &'static [(u8,u8)], features: Option<&'static [DeviceFeature]> }`.
`generated.rs` holds `pub static MODELS: &[(&str, DeviceModel)]` sorted by product ID (binary search).
`bit_for_type(t)` = smallest bit in `set_bits` with value t, else smallest in `reported_bits`. `GENERIC` and
`GENERIC_REALME` and `model(product_id, device_name)` (realme if name contains "realme" or pid ends "12"/"52")
from `Devices.swift:48-72`. Capability bits: gestures 3, findBuds 5, lowLatency 7, noiseControl 8, equalizer 10,
customEQ 34.

### 2.3 Controller (port of `BudsController.swift`)

```rust
pub trait Platform: Send + Sync + 'static {
    /// Paired Bluetooth devices (any kind); core filters with looks_like_buds().
    fn paired_devices(&self) -> impl Future<Output = Vec<BtDevice>> + Send;
    /// Open the control channel. Walks this OS's candidate list (2.4); `sink` receives data/close.
    fn open(&self, dev: &BtDevice, sink: LinkSink, log: LogFn) -> impl Future<Output = Result<Box<dyn Link>, OpenError>> + Send;
    /// Ask buds that are connected elsewhere to connect to this computer (auto reconnect).
    fn page(&self, dev: &BtDevice, log: LogFn) -> impl Future<Output = Result<(), String>> + Send;
    fn connect_budget(&self) -> Duration;   // connecting watchdog: mac 8 s, Windows/Linux 20 s
    fn host_label(&self) -> &'static str;   // "PC" | "Mac" | "computer"
}
pub struct BtDevice { pub address: [u8; 6], pub name: Option<String>, pub connected: bool, pub has_control_service: bool }
pub trait Link: Send {
    fn channel(&self) -> Option<u8>;
    fn connect_hint(&self) -> Option<String>;   // which candidate worked (stored in settings.connectHints)
    fn write(&self, b: &[u8]) -> std::io::Result<()>;
    fn close(&self);
}
```

- **Cancelling opens**: each open runs in a spawned task tagged with an `attempt` id. Watchdog / reconnect bumps
  the attempt id (and aborts the task). A `LinkOpened` result for an old attempt must `close()` the link it
  carries, so a late success never leaks a socket.

- `Controller<P>` owns all state from `BudsController.swift:27-160`; `run()` loops over `Msg`:
  user actions (`SetMode(mode, Option<level>)`, `CycleMode`, `SetNoiseBit`, `SetLowLatency`, `SetEq`,
  `SetCustomEq(Vec<i32>)`, `FindBuds(bool)`, `SetGesture`, `Reconnect`, `Refresh`, `QueryHosts`,
  `SendRaw(u16, Vec<u8>)`, `ScanQueries`, `Shutdown`), platform results (`DevicesPolled(Vec<BtDevice>)`,
  `LinkOpened{attempt, Result}`, `Data{link_id, bytes}`, `LinkClosed{link_id}`, `PageDone`), and
  `Timer(TimerKind, generation)`.
- **Timers** (`controller/timers.rs`): `schedule(kind, after)` bumps a per-kind generation and spawns
  `sleep(after)` → `Msg::Timer(kind, gen)`; `cancel(kind)` bumps the generation; stale generations are ignored.
  This replaces `DispatchWorkItem.cancel()` and `Timer.invalidate()`. Use `tokio::time` so tests can `pause()`.
- **Link identity**: every opened link gets an increasing `link_id`; data/close for a non-current id is dropped
  (replaces `rfcommChannel === channel`).
- **Device events by polling** (uniform on all OSes): the health tick (every 2 s) runs `paired_devices()`,
  diffs `connected` per buds address and synthesizes *device connected* (→ schedule connect in 3 s, as
  `deviceDidConnect`) and *disconnected* (→ `closeChannel`, clear batteries, retry) handling. Also the upstream
  health rule: if `link == Searching` and a connected buds exists → connect.
- **Sleep/resume** without OS hooks: on each health tick compare `SystemTime::now()` with the previous tick; a gap
  > 10 s means the computer slept → after 2 s: `refresh()` if connected else `reconnect()` (upstream lines 171-178).
- Port **verbatim**: `supports()`, `has_more_controls`, `eq_choices`, `gesture_actions(current)` (realme list
  vs others, noise control/low latency conditional), `budsBattery` (min of L/R), `setMode` (ANC level fallback:
  current level → settings `lastAncLevel` → first available; type → bit via catalog; optimistic UI; remember),
  `cycleMode` (order ANC → Transparency → Off filtered by available), `setNoiseBit` (stale noise query tracking),
  `setGameMode`, `setEQ`, `setCustomEQ`, `findBuds` (30 s auto-stop), `setGesture`, queued mode while
  disconnected (+ reconnect if searching), `didOpen` sequence (hello, productID, 1.5 s → applyQueuedMode,
  subscribe `03 01 02 03`, wear, multipoint, EQ, EQ list `01 05`, switch `01 06`, gestures, refresh),
  `expectReply` (3 s → reconnect if nothing received since send), retry delays `[1,2,3,5,8]` s,
  poll refresh every 30 s, `resetDeviceState` on a different address, remembered settings per address
  (`restoreNoise`/`restoreLowLatency` flags, "picked while disconnected wins"), page rate limit (≥15 s apart,
  in-flight expires after 30 s, only if `autoConnect`).
- `looks_like_buds`: `has_control_service` OR name (lowercase) contains oneplus/oppo/enco/realme/dizo OR address is
  in `settings.known_buds` (addresses we have successfully talked to; covers renamed buds on Windows where SDP
  isn't visible). `paired_buds()` prefers `settings.last_buds_address`; `find_buds()` = connected + looks like
  buds, preferring `has_control_service`.
- **Snapshot** (`serde` camelCase + `ts-rs` export to `src/lib/bindings/`): link, deviceName, productId, address,
  channel, mode, level, noiseBit, left/right/caseBattery {level, charging}, budsBattery, lowLatency, eqId,
  customEqs, activeCustomEq, eqChoices [{id,name}], gestures {left/right: {doubleTap,tripleTap,hold}},
  gestureOptions (same shape, list of actions with title), findingBuds, hosts, wear, capabilitiesCount,
  modelName, inCatalog, availableModes, availableLevels, supports {noiseControl,equalizer,customEq,lowLatency,
  findBuds,gestures}, hasMoreControls, layout [{bit, modeType, name}], hostLabel. Publish only when changed.
- `ControllerHandle { tx, state: watch::Receiver<Snapshot> }` is `Clone` and is what the app, hotkeys, IPC and CLI use.

### 2.4 Bluetooth backends

**Windows (`buds-bt/windows`, `windows` crate; features `Win32_Foundation`, `Win32_Networking_WinSock`,
`Win32_Devices_Bluetooth`, later COM/audio for 9.2):**
- `paired_devices()`: `BluetoothFindFirstDevice/NextDevice/FindDeviceClose` with `fReturnAuthenticated`,
  `fReturnRemembered`, `fReturnConnected` = TRUE, `fReturnUnknown` = FALSE, **`fIssueInquiry = FALSE`** (no radio
  scan), `hRadio = null`; map `Address.ullLong` (low 48 bits) → `[u8;6]`, `szName`, `fConnected`. Run in
  `spawn_blocking`. `has_control_service = false` (Windows doesn't expose vendor SDP records cheaply; name +
  `known_buds` cover it).
- RFCOMM link: `WSAStartup(0x0202)` once (`std::sync::Once`); `socket(AF_BTH=32, SOCK_STREAM, BTHPROTO_RFCOMM=3)`;
  `connect` with `SOCKADDR_BTH { addressFamily: AF_BTH, btAddr, serviceClassId, port }`. Blocking connect runs on
  its own thread; timeout = close the socket from the waiting side (`closesocket` aborts the connect). After
  success, `getpeername` → `SOCKADDR_BTH.port` = channel (for the Developer view). Reader thread loops
  `recv(1024)` → `sink.data(bytes)`; 0 or error → `sink.closed()`. `write` loops `send` until all bytes sent.
  `close` = `shutdown(SD_BOTH)` + `closesocket`.
- Candidate order in `open()` (mirrors upstream "enco/UUID records → SPP → fallback channels"):
  0. If `settings.connect_hints[address]` has a candidate that worked before, try it first.
  1. GUID `0000079A-D102-11E1-9B23-00025B00A5A5` (serviceClassId set, `port = 0` → Windows does the SDP lookup;
     verify against the `SOCKADDR_BTH` docs).
  2. GUID `00001107-D102-11E1-9B23-00025B00A5A5`.
  3. Only if 1 **and** 2 failed with `WSASERVICE_NOT_FOUND (10108)`: SPP `00001101-0000-1000-8000-00805F9B34FB` (realme).
  4. Only if every GUID was "not found": channels 15, 12, 13 (zero GUID, `port = n`).
  Any other error (timeout 5 s per GUID / 3 s per channel, host down) → stop and return `Err` (controller retries).
  Log each step like upstream ("opening RFCOMM channel 15", "channel 15 timed out"). Return the winning candidate
  so the controller can store it in `connect_hints`.
- `page()` (Phase 3 version): a GUID-1 connect with a 5 s timeout, closed immediately, to page the buds.
  Phase 9 adds the audio-endpoint reconnect (KS one-shot) which is what actually brings audio back.
- `sdp.rs` (Phase 9, diagnostics for `budsctl sdp`): `WSALookupServiceBeginW/NextW` on the device
  (`NS_BTH`, context `"(AA:BB:CC:DD:EE:FF)"`, service class L2CAP `0x0100`, `LUP_RETURN_NAME|LUP_RETURN_ADDR`)
  → print service name + RFCOMM channel. If a record named like "enco" appears, prefer its channel in `open()`.

**Linux (`buds-bt/linux`, crate `bluer` with features `bluetoothd`, `rfcomm`):** `paired_devices` from the default
adapter (name, `is_connected`, `uuids()` → `has_control_service` if a HeyMelody UUID is present). `open`: for each
HeyMelody UUID the device advertises (else SPP for realme), connect through a BlueZ **client profile**
(`register_profile` with role Client + `Device::connect_profile(uuid)`, accept the `ConnectRequest` → `rfcomm::Stream`);
fallback `rfcomm::Stream::connect(SocketAddr{addr, channel})` on 15, 12, 13. `page` = `Device::connect()` (connects
audio profiles too). Verify the exact bluer API against its docs/examples for the pinned version.

**macOS (`buds-bt/macos`, crates `objc2`, `objc2-foundation`, `objc2-io-bluetooth`, `dispatch2`, `block2`):** port
`BudsController.swift:345-660` connection code 1:1 (SDP records named "enco" + UUID records → SPP → fallback
channels, `openRFCOMMChannelAsync`, 3 s per channel, adopt already-open channels, `openConnection` page with
timeout 8000). All IOBluetooth calls happen on the main GCD queue; the delegate class (`define_class!`) forwards to
`LinkSink`. If `objc2-io-bluetooth` lacks needed APIs, fall back to a tiny Swift static library exposing
`@_cdecl` functions (built in `build.rs` via `swift-rs`), reusing upstream code.

### 2.5 Settings, log, IPC, CLI
- `Settings` (serde JSON, camelCase, every field `#[serde(default)]`): autoConnect=true, rememberSettings=false,
  showHud=true, developerMode=false, moreExpanded=false, gestureSide="left", lastAncLevel, lastBudsAddress,
  knownBuds[], connectHints{addr→candidate}, shortcuts{cycle|anc|transparency|off → accelerator string},
  remembered{addr→{mode, level, lowLatency}}. File: `dirs::config_dir()/BudsControl/settings.json`
  (Windows `%APPDATA%\BudsControl`). Atomic save (write `.tmp` then rename). Shared as `Arc<RwLock<Settings>>`.
- `PacketLog`: ring buffer of 600 entries {time, direction sent/received/info, text}, `paused` flag (pause stops
  the list, never the file), file `BudsControl.log` in `dirs::data_local_dir()/BudsControl` (macOS
  `~/Library/Logs`), truncated at startup when > 2 MB, line format `yyyy-MM-dd HH:mm:ss.SSS → AA 0B …`.
  Broadcasts new entries (app batches them to the UI every 100 ms).
- `cli.rs`: `execute(handle, args, out: &mut dyn Write) -> i32` implements every upstream CLI mode with the same
  output format (`main.swift:31-173`): actions `anc|transparency|off|smart|max|moderate|mild|adaptive|cycle|status`,
  `eq=<id>`, `custom=g1,g2,…`, `lowlatency=on|off`, `find=on|off`, `gesture=<left|right>,<doubleTap|tripleTap|hold>,<action>`,
  `watch [secs]`, `scan [label]`, `raw CMD:PAYLOAD …`, `sdp`. Also accept upstream spellings (`--cli anc`, `--watch 30`).
- IPC (`interprocess` local sockets): Windows named pipe `budscontrol-<username>`; Unix socket file
  `$XDG_RUNTIME_DIR/budscontrol.sock` (macOS `$TMPDIR`), mode 0600. Request: one JSON line `{"args":[…]}`;
  response: JSON lines `{"line":"…"}` … then `{"exit":N}`. The app runs the **same** `cli::execute` against its live
  controller. `budsctl` tries IPC first (unless `--direct`), else runs its own controller with the native platform.

### 2.6 UI spec (Windows 11 equivalent of the popover)
- **Tray**: left click toggles the flyout; right click shows a native menu: Noise Cancellation / Transparency /
  Off (check items, disabled when not connected or not available), separator, Settings…, Reconnect, separator,
  Quit Buds Control. Middle click = cycle mode. Tooltip: `Buds Control · <name>\nL 80% · R 82% · Case 45%\nANC · Max`
  (≤127 chars). Icon = monochrome earbuds glyph inside a battery ring (arc clockwise from 12 o'clock, track 28%
  alpha, glyph 45% alpha when disconnected); white on dark taskbar, black on light taskbar
  (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme`); size =
  round(16 × taskbar monitor scale).
- **Flyout** (`label "flyout"`, created hidden at startup for instant open): 300 logical px wide, height follows
  content, no decorations, transparent, shadow (Win11 rounds corners), always on top, skip taskbar, Acrylic
  (Win11/10), hides on blur and on Esc. Anchored to the tray icon rect: centred horizontally on the icon, clamped
  into the monitor work area with 12 px margin, on the taskbar side (bottom taskbar → bottom edge = work-area
  bottom − 12). Growing content keeps the anchored edge fixed.
  Layout (top→bottom, 12 px padding, 10 px gaps): **Header** (34 px battery ring with glyph, device name 14 px
  semibold, subtitle 12 px secondary `"80% · ANC · Max"` / "Connecting…" / "Not connected", then the More chevron
  (24 px circle, rotates 180°, only when connected and `hasMoreControls`) and the "…" button opening a native
  menu Settings… / Reconnect / Quit) → **Mode segmented control** (Off / Transparency / ANC icons; selected
  segment shows icon + short title; accent-filled sliding pill, 220 ms overshoot ease) → **ANC level segmented**
  (only when mode = ANC and levels exist; 24 px tall; labels sized to text with even gaps) → **Battery row**
  (L / R / Case glyph + %, "—" when unknown, green bolt when charging, red when ≤20% and not charging) → **More
  panel** (collapsible: Equalizer card [segmented if ≤4 names fit in 250 px, else picker; custom EQ vertical band
  sliders 64 px tall with `+3`/`0` labels and `1k` style Hz labels, commit on pointer-up/keyboard], Low latency
  card ["For games and video", toggle switch], Find earbuds card [subtitle "Loud — take them out of your ears
  first" / "Playing a sound…", Play/Stop button red when playing], Gestures card [Left/Right segmented, one picker
  row per reported gesture]). **Disconnected**: card "Buds not connected to this PC" + Retry, or spinner +
  "Connecting…". Pickers use Tauri native popup menus (`@tauri-apps/api/menu`), with a blur-guard flag so opening a
  menu doesn't hide the flyout.
- Expand animation: grow the window to the final height first, then animate content (CSS `grid-template-rows`
  0fr→1fr, 250 ms); collapse: animate, then shrink the window.
- Theme tokens (`tokens.css`, light/dark via `prefers-color-scheme`): font `"Segoe UI Variable Text","Segoe UI",
  system-ui,-apple-system,Cantarell,sans-serif`; `--accent` from the OS (Rust `get_accent_color`: Windows
  `HKCU\Software\Microsoft\Windows\DWM\AccentColor` ABGR DWORD, macOS `NSColor.controlAccentColor`, else default
  `#0067C0`); subtle fill `rgba(0,0,0,.04)`/`rgba(255,255,255,.06)`; stroke `rgba(0,0,0,.06)`/`rgba(255,255,255,.08)`;
  danger `#C42B1C`; success `#0F7B0F`/`#6CCB5F`; card radius 8 px; solid fallback background `#F3F3F3`/`#202020`
  when no window effect is available. Respect `prefers-reduced-motion`.
- Icons: Fluent UI System Icons (MIT) path data in `Icon.svelte` (pick closest; verify names in
  github.com/microsoft/fluentui-system-icons): prohibited (Off), an ear/"person sound" style icon (Transparency),
  headphones-sound-wave style (ANC), flash (charging), options/sliders (EQ), games (low latency),
  location-ripple/radar (find), tap-single (gestures), settings, more-horizontal, chevron-down, arrow-sync (cycle),
  keyboard, wrench, info, arrow-up/arrow-down. Earbuds, left/right bud and case glyphs are our own SVG
  (same geometry as `buds-art`). Attribute in THIRD_PARTY_NOTICES.
- **Settings window** ("Buds Control Settings", 600×640, Mica on Win11, created on demand, destroyed on close): tabs
  General / Shortcuts / Developer (only when developer mode on). General: device card (glyph, name, "Connected ·
  model 065414", Reconnect), toggles Launch at login, Reconnect buds to this PC automatically (+ description), Remember
  settings between connections (+ description), Show HUD when a shortcut changes mode, Show developer options;
  Windows-only tip about pinning the icon from the tray overflow. Shortcuts: four recorder rows (Cycle modes, Noise
  Cancellation, Transparency, Off) with conflict text "Used by another app — pick a different combination.".
  Developer: Device (name, address, RFCOMM channel, product ID, active noise bit), Noise modes (every layout bit,
  Apply / ✓), Experimental game-mode toggle, Paired devices (multipoint, "(this PC)"), Raw command (cmd hex, payload
  hex, Send; Product ID / Battery / Noise mode / Scan 0x0101–0x0120), Packet log (time, ↑ blue / ↓ green / ⓘ grey,
  selectable, auto-scroll unless scrolled up, Pause / Copy / Clear).
- **HUD** (`label "hud"`, created on first use, kept hidden): transparent, click-through
  (`set_ignore_cursor_events(true)`), not focusable, topmost, skip taskbar; pill with 44 px icon circle (accent
  tint, neutral for Off), title + "<Level> level" or device name; bottom-centre of the monitor under the cursor,
  120 px above the work-area bottom; fade in 150 ms, visible 1.4 s, fade out 300 ms; repeated presses restart.

### 2.7 Constants (one `consts.rs`, names as below)

| Name | Value | Upstream |
|---|---|---|
| RETRY_DELAYS | 1, 2, 3, 5, 8 s | `retryDelays` |
| POLL_INTERVAL | 30 s | `pollTimer` |
| HEALTH_INTERVAL | 2 s (upstream 3 s; we also poll devices here) | `healthTimer` |
| CONNECT_AFTER_DEVICE_UP | 3 s | `deviceDidConnect` |
| CONNECTING_WATCHDOG | `platform.connect_budget()` (8 s mac, 20 s else) | `armConnectingWatchdog` |
| REPLY_WATCHDOG | 3 s | `expectReply` |
| APPLY_QUEUED_MODE_DELAY | 1.5 s | `didOpen` |
| FIND_AUTO_STOP | 30 s | `findBuds` |
| PAGE_MIN_INTERVAL / PAGE_INFLIGHT_EXPIRY | 15 s / 30 s | `pageBuds` |
| RESUME_GAP / RESUME_DELAY | 10 s / 2 s | wake observer |
| SCAN_STEP | 80 ms (CLI scan 90 ms after 4 s) | `scanQueries` |
| LOG_RING / LOG_FILE_MAX | 600 / 2 MB | `PacketLog` |
| HUD_IN / HUD_HOLD / HUD_OUT | 150 ms / 1.4 s / 300 ms | `HUD.swift` |

---

## 3. Rules for the implementer (read before every phase)

1. Do phases in order. A phase is done only when **every gate item passes**; paste the gate command output into the
   phase's commit message body or `docs/PROGRESS.md`. Never start the next phase with a failing gate.
2. When porting, open the upstream Swift file in `reference/upstream/Sources/` next to your Rust file and port
   branch-by-branch. Keep upstream comments that explain *why* (they encode hardware quirks).
3. No placeholders: no `todo!()`, `unimplemented!()`, empty handlers or "TODO" left at a gate.
4. Always run before committing: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `npm run check` (svelte-check + tsc). From Phase 4 also `npm run tauri build -- --debug`.
5. Pin versions at Phase 0 (latest stable Tauri 2.x, `@tauri-apps/*` 2.x, Svelte 5, Vite, tokio 1, serde,
   `windows`, `bluer`, `objc2*`, tiny-skia, clap 4, ts-rs, interprocess 2, dirs, chrono). Commit `Cargo.lock` and
   `package-lock.json`. Don't upgrade mid-project. When an API name differs from this plan, check docs.rs /
   tauri.app for the pinned version and note the difference in `docs/PROGRESS.md`.
6. Svelte 5 runes only (`$state`, `$derived`, `$props`, `$effect`); TypeScript strict.
7. Everything sent/received/connection-related is logged through the packet log (upstream logs every frame).
8. Commit per phase on branch `main` with the message `phase N: <summary>`.
9. Hardware steps (anything needing the real OnePlus Buds 4, UAC prompts, sign-out/in, sleep/wake) are done by the
   project owner: list exactly what to do and what to look for, then wait for their result before marking the gate.

---

## 4. Phases

### Phase 0 — Toolchain and skeleton
1. Install: `winget install Rustlang.Rustup` → `rustup default stable` (target `x86_64-pc-windows-msvc`);
   `winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"`
   (needs the project owner's UAC approval). Open a new shell; verify `rustc -V`, `cargo -V`, and that `cargo build` of a
   hello crate links (MSVC `link.exe` found).
2. `git init`; `.gitignore` (target/, node_modules/, dist/, reference/, src-tauri/binaries/, *.log).
3. `git clone https://github.com/amarsakore/BudsControl reference/upstream` and
   `git -C reference/upstream checkout 298d4527337b86052a7ebac8f1c2f52df451239f`.
4. Scaffold Tauri 2 + Svelte-TS (`npm create tauri-app@latest` into a temp dir, then move files so the layout in §1
   holds), Vite multi-page (`flyout.html`, `settings.html`, `hud.html`). Create workspace crates `buds-core`,
   `buds-bt`, `buds-art`, `budsctl`, `icongen` (compiling stubs).
5. `LICENSE`: MIT with `Copyright (c) 2026 amarsakore` (upstream) and a second line for this port; `THIRD_PARTY_NOTICES.md`
   copied from upstream (RawS-Music Apache-2.0, oppo-pods MIT) + Fluent UI System Icons MIT; README stub with credits.
6. Set `tauri.conf.json`: productName "Buds Control", identifier placeholder `io.github.nilcodes.budscontrol`
   (user may change), no default window (windows are created from Rust).

**Gate 0:** `cargo build --workspace` ✓ · `npm run check` ✓ · `npm run tauri dev` opens a test window ✓ ·
`git log` shows commit `phase 0`.

### Phase 1 — Protocol, parsers, catalog (`buds-core`, pure functions)
1. `types.rs`: NoiseMode, AncLevel (modeType 3/4/7/8/10), ModeType helpers + names, EqPreset, CustomEq,
   EarSide, Gesture, GestureAction (incl. realme assistant 0x04), Brand, DeviceFeature (+ capability bits),
   Battery, Wear, Host, LinkState — all `serde` + `ts-rs`.
2. `protocol.rs`: command constants, `Frame::encode`, `FrameParser::feed` (LEB128; skip bytes until 0xAA;
   keep incomplete tail).
3. `parse.rs` (pure fns returning typed values: battery, capabilities, noise→bit, switches, eq list, gestures,
   wear, hosts, product id) and `encode.rs` (noise mask payload, custom EQ payload, gesture payload).
4. `tools/convert_catalog.py` reads `reference/upstream/Sources/DeviceCatalog.swift` with regexes and writes
   `catalog/generated.rs` (header comment with sources + "generated, do not edit"); `catalog/mod.rs` with lookup,
   `GENERIC`, `GENERIC_REALME`, `bit_for_type`.
5. `docs/PROTOCOL.md`.

**Gate 1 — unit tests must exist and pass:** frame encode of `0x0106` seq 1 = `AA 07 00 00 06 01 01 00 00`;
parser: one frame, frame split over 3 chunks, two frames in one chunk, junk before `AA`, 2-byte LEB128 length,
truncated tail kept; battery (charging bit, case 0 → None); capabilities bit 34; noise for Buds 4 (`065414`):
reported bit 8 → Transparency, bit 3 → Off, bit 4 → ANC Max, bit 11 → Adaptive; Buds Pro 3 (`064014`) bit 13 →
Moderate; unknown bit ignored; set-noise payload for bit 11 = `01 01 00 08`; custom EQ round trip with negative
gains; gestures ignore ctx 06; hosts reversed address; catalog has **105** models; `bit_for_type(Off)` on Buds 4 = 0,
Transparency = 2; realme fallback for unknown pid ending `12`. `cargo clippy -D warnings` ✓.

### Phase 2 — Controller actor + fake buds
1. `platform.rs` (trait §2.3), `settings.rs`, `packet_log.rs`, `controller/*` porting `BudsController.swift`
   completely (except the macOS-specific connection code which goes to Phase 12), `snapshot.rs`.
2. `fake.rs` (feature `fake`): `FakePlatform` + `FakeBuds` simulating a Buds 4: answers hello (capability bitmap
   with bits 3,5,7,8,10,34), product ID `065414`, battery, noise (reports Transparency as bit 8), switches, EQ id,
   one custom EQ (6 bands 62/250/1k/4k/8k/16k Hz, −6…+6), gestures for both sides, wear, two hosts; acks set
   commands and sends the matching push ~0.5 s later; can be told to drop replies, close the link, disconnect, or
   appear "connected to phone only".
3. `cli.rs` (status/actions/watch/scan/raw output identical to upstream) — usable with the fake platform.
4. Add `.github/workflows/ci.yml`: matrix windows/ubuntu/macos → `cargo test -p buds-core --features fake`, fmt, clippy.

**Gate 2 — controller tests with `tokio::time::pause()`:** connects and reaches Connected with productId
`065414`, battery, mode; `SetMode(Anc, None)` picks last level → settings level → first level; optimistic UI then no
flicker from a stale noise reply; retry delays observed 1,2,3,5,8,8 s while no device; reply watchdog reconnects
after 3 s silence; connecting watchdog fires; mode set while disconnected is sent after connect (after productId);
find auto-stops at 30 s; remember-settings restores ANC Max after reconnect and does nothing if unchanged;
different address resets state; device-connected transition connects after 3 s; page at most every 15 s and only
when autoConnect; resume gap triggers refresh. `budsctl`-style `cli::execute(["status"])` against fake prints the
upstream status block. CI green on all three OSes.

### Phase 3 — Windows Bluetooth backend + `budsctl` (first hardware milestone)
1. `buds-bt/windows` per §2.4 (devices, RFCOMM link, candidate walk, connect hints, page v1).
2. `crates/budsctl`: `clap`; `budsctl [--direct] [--fake] <action…>`; direct mode builds a tokio runtime, the native
   platform, the controller, then `cli::execute`. (IPC comes in Phase 9.)
3. `docs/TESTING.md`: hardware checklist (see §6) with a results column.

**Gate 3 (Buds 4 paired in Windows Settings):** `budsctl status` prints `device:  OnePlus Buds 4 [065414] connected`,
battery, current mode, EQ, low latency, gestures; `budsctl anc|transparency|off|max|mild|adaptive` audibly switch;
`budsctl watch 30` shows pushes when a bud is taken out / gesture used; `budsctl raw 0106` logs a received frame whose
command bytes are `06 81` (reply 0x8106);
`eq=…`, `custom=0,2,4,2,0,-2`, `lowlatency=on`, `find=on` (loud!), `gesture=left,doubleTap,next` work; with buds in the
case the command reports not connected and exits 1. Record which candidate connected (expect GUID or channel 15) in
`docs/PROGRESS.md`. **Stop and report to the project owner if no candidate connects.**

### Phase 4 — Tauri shell: controller, tray, flyout window, bridge
1. `lib.rs` setup: plugins (single-instance first, global-shortcut, autostart, clipboard-manager), load settings,
   create platform (`BUDS_FAKE=1` → fake), spawn controller on `tauri::async_runtime`, forward `watch` snapshots as
   event `buds://state`, log batches as `buds://log`, settings as `buds://settings`.
2. Commands (`commands.rs`): `get_state`, `get_settings`, `update_settings(patch)`, `set_mode(mode, level?)`,
   `cycle_mode`, `set_noise_bit`, `set_low_latency`, `set_eq`, `set_custom_eq(gains)`, `find_buds(start)`,
   `set_gesture(side, gesture, action)`, `reconnect`, `send_raw(cmd, payloadHex)`, `scan_queries`, `get_log`,
   `clear_log`, `set_log_paused`, `get_accent_color`, `flyout_resize(height)`, `set_menu_open(bool)`,
   `open_settings`, `quit`. `src/lib/api.ts` wraps each with typed functions; `stores.ts` holds `$state` stores fed
   by events + initial `get_*`.
3. Tray (static icon for now) with menu and click handling per §2.6; flyout window per §2.6 with positioning
   (`positioning.rs`: tray rect from the click event + monitor work area; Linux/no-rect → corner nearest the cursor).
   Blur race: remember when the flyout was hidden by blur; a tray click within 300 ms after that must not re-open it.
4. Manual launch shows the flyout once; `--background` (autostart) shows nothing. Second launch (single-instance) →
   show flyout.

**Gate 4 (run with `BUDS_FAKE=1` and with real buds):** tray icon visible; left click toggles flyout at the right
place (taskbar bottom, then move taskbar/monitor scale 150% and re-check); click outside and Esc hide it; tray menu
switches modes; a temporary debug view in the flyout shows live JSON snapshot changing when a bud is removed;
starting a second instance focuses the first.

### Phase 5 — Flyout UI
Build all components from §2.6 (`Segmented`, `Card`, `Row`, `ToggleSwitch`, `PickerButton`, `Button`,
`BatteryRing`, `EarbudsGlyph`, `Icon`, `EqBands`/`BandSlider`, `Spinner`) and the flyout sections. Remove the debug
view. Height changes go through `flyout_resize` (ResizeObserver on the root). Keyboard: Tab order, arrows inside
segmented controls, Enter/Space activate; ARIA roles (radiogroup, switch, slider) and labels like upstream.

**Gate 5:** `npm run check` ✓; fake mode visual review in light and dark mode at 100% and 150% scale (screenshots
saved under `docs/screenshots/`); hardware: every flyout control works on Buds 4 (§6 items 2–10); changing ANC level
by long-press on the bud updates the flyout within ~1 s; opening the flyout feels instant (window pre-created).

### Phase 6 — Dynamic tray icon, tooltip, menu state
1. `buds-art`: `draw_glyph(canvas, rect, color, alpha)` (two mirrored earbuds: oval head + rounded stem rotated 14°,
   as `MakeIcon.swift`), `draw_ring(canvas, rect, level, color, track_alpha, line_width)`.
2. `tray_render.rs`: RGBA → `tauri::image::Image`; re-render when budsBattery/link/taskbar theme/scale changes
   (theme checked on each snapshot and every 10 s). Tooltip and menu check states updated from snapshots.

**Gate 6:** icon ring matches battery, dims when disconnected, flips colour when Windows switches light/dark taskbar,
crisp at 100%/150%/200%; tooltip text correct; menu checkmarks follow mode; no GDI handle growth over 10 minutes of
updates (Task Manager → GDI objects column stable).

### Phase 7 — Settings window + launch at login
Settings window and three tabs per §2.6; `update_settings` persists immediately and is pushed to the controller
(autoConnect, rememberSettings) and UI; "Launch at login" uses `tauri-plugin-autostart` with args `["--background"]`
and reads the real state (`is_enabled`) each time the tab opens. Packet log view: initial `get_log` + appended
batches, max 600 rows, Copy via clipboard plugin.

**Gate 7:** toggles survive app restart (check `%APPDATA%\BudsControl\settings.json`); launch at login creates/removes
`HKCU\…\Run` value and after sign-out/in the app starts hidden in the tray; Developer tab: raw send, scan, noise-bit
Apply, hosts list shows this PC and the phone, Pause/Copy/Clear work; closing the window frees it (memory drops).

### Phase 8 — Global shortcuts + HUD
1. `hotkeys.rs`: register from `settings.shortcuts` with `tauri-plugin-global-shortcut`; failures → `conflicts` set
   sent to UI; `set_recording(true)` unregisters all, `false` re-registers (upstream `setRecording`). Handler calls
   `perform(action)`: cycle / anc / transparency / off, then shows the HUD if `showHud` and mode is known.
2. `ShortcutRecorder.svelte`: click → "Type shortcut…"; keydown builds accelerator from `KeyboardEvent.code` +
   Ctrl/Alt/Shift/Meta; Esc cancels, Backspace clears; requires a modifier unless F1–F24 (else shake/beep); display
   `Ctrl+Alt+N` on Windows/Linux, `⌃⌥⇧⌘N` on macOS.
3. `hud.rs` + `src/hud/*` per §2.6.

**Gate 8:** shortcut works while another app is focused (e.g. a full-window browser); HUD appears bottom-centre on
the monitor with the cursor and never takes focus or blocks clicks; registering a combo already used by another app
shows the conflict text; recording doesn't trigger the old shortcut; shortcuts persist across restart.

### Phase 9 — Reconnect polish, IPC, diagnostics
1. **Windows audio reconnect** (`audio_reconnect_win.rs`, `windows` COM features `Win32_Media_Audio`,
   `Win32_Media_KernelStreaming`, `Win32_System_Com`): enumerate render+capture endpoints (all states) via
   `IMMDeviceEnumerator`; for each, `IDeviceTopology` → connector 0 → `GetConnectedTo` → `IPart` →
   `GetTopologyObject` → `GetDeviceId`; if the id contains the buds address as 12 lowercase hex digits, open that
   device with `IMMDeviceEnumerator::GetDevice(id)` → `Activate::<IKsControl>` → `KsProperty` with
   `KSPROPSETID_BtAudio {7fa06c40-b8f6-4c7e-8556-e8c33a12e54d}`, id 0 (`KSPROPERTY_ONESHOT_RECONNECT`),
   `KSPROPERTY_TYPE_GET`. Use it as `page()`'s first step; fall back to the RFCOMM page.
2. IPC server in the app + client in `budsctl` per §2.5; `budsctl` without `--direct` forwards when the app runs.
3. `sdp.rs` + `budsctl sdp` per §2.4; if an "enco" record is found, use its channel first in `open()`.

**Gate 9:** with the phone nearby: put buds in case, take them out so they join the phone only → within ~20 s the app
brings them back and audio plays on the PC (Settings toggle off → it doesn't); sleep the PC 1 min and wake →
Connected again or refreshed without clicking Retry; `budsctl anc` while the app runs changes mode and the flyout
updates; `budsctl status` prints the same block via IPC; `budsctl sdp` lists services.

### Phase 10 — Windows packaging and release
1. `crates/icongen`: render 1024 px app icon (blue→indigo rounded square, white 75% ring, two earbuds, per
   `MakeIcon.swift`), then `npx tauri icon app-icon.png` → `src-tauri/icons/*`.
2. `budsctl` as a Tauri sidecar: `scripts/build-sidecar.mjs` builds `budsctl --release` and copies it to
   `src-tauri/binaries/budsctl-<target-triple>.exe`; `bundle.externalBin = ["binaries/budsctl"]`;
   `beforeBuildCommand` runs it.
3. NSIS: `bundle.targets = ["nsis"]`, `installMode: "currentUser"`, WebView2 `downloadBootstrapper`, Start menu
   shortcut, uninstaller; an NSIS hooks file (`NSIS_HOOK_POSTUNINSTALL`) deletes the `HKCU\…\Run` autostart value.
4. `scripts/package-windows.ps1`: `npm ci`, `npm run tauri build`, then zip `BudsControl.exe`, `budsctl.exe`,
   `README.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md` → `dist/BudsControl-<ver>-windows-x64-portable.zip`.
5. `.github/workflows/release.yml` (tag `v*`): Windows job builds installer + portable zip and attaches them.
6. README: features, install (installer / portable, SmartScreen "More info → Run anyway" since unsigned),
   tray-overflow pinning tip, CLI usage, how it works, credits (upstream author, contributors, protocol references),
   disclaimer.

**Gate 10:** in Windows Sandbox (install check only; it has no Bluetooth): installer installs without admin, app
starts in the tray, uninstall removes it and the autostart value; on the real PC: portable zip runs from any folder; `budsctl.exe` next to the app works; full §6 checklist
passes on the release build; idle CPU ≈ 0% over 5 minutes; tag-triggered workflow produces both artifacts.

### Phase 11 — Linux
1. `buds-bt/linux` per §2.4.
2. App specifics: tray via libayatana-appindicator (no click events on Linux → tray menu gets "Open Buds Control"
   first, and the menu is the primary entry); flyout opens near the cursor (X11) / centred (Wayland can't
   position windows); no window effects → solid background tokens; accent default; HUD → desktop notification
   (`notify-rust`, reuse one notification id); autostart → `~/.config/autostart` via the plugin; global shortcuts
   work on X11 only → Shortcuts tab shows "On Wayland, bind `budsctl cycle` in your desktop's keyboard settings".
3. Packaging: AppImage + .deb (+ `budsctl` sidecar); CI job on ubuntu-22.04 with `libwebkit2gtk-4.1-dev
   libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev`.

**Gate 11:** CI builds AppImage/.deb; fake mode UI works in an X11 session; on real hardware (someone with Linux +
HeyMelody buds) `budsctl status` and the §6 core items pass — if no tester is available, mark hardware items
"pending" in `docs/TESTING.md` and tell the project owner.

### Phase 12 — macOS
1. `buds-bt/macos` per §2.4 (main-queue IOBluetooth, delegate class, upstream connection logic).
2. App specifics: `ActivationPolicy::Accessory` (no Dock icon), template tray icon (`icon_as_template(true)`),
   flyout below the menu-bar icon with `Effect::Popover` vibrancy (`macOSPrivateApi: true`), HUD visible on all
   spaces, `Info.plist` with `NSBluetoothAlwaysUsageDescription` (upstream text) and `LSUIElement`, autostart via
   LaunchAgent, ad-hoc signing (`signingIdentity "-"`), `.dmg`.
3. `budsctl` on macOS runs its controller on worker threads while the main thread runs the run loop (IOBluetooth
   needs it).

**Gate 12:** CI builds the `.app`/`.dmg`; on a Mac with buds the §6 items pass (else mark pending and tell the project owner).

---

## 5. Verification summary (how to check end-to-end)

- Automated: `cargo test --workspace --features buds-core/fake`, `cargo clippy … -D warnings`, `npm run check`, CI matrix
  on Windows/Linux/macOS, `npm run tauri build`.
- No-hardware UI review: `BUDS_FAKE=1 npm run tauri dev` (all states reachable: connected, connecting, disconnected,
  charging, low battery, finding, long EQ names → picker).
- Hardware (Windows + OnePlus Buds 4): the `docs/TESTING.md` checklist below, re-run at Gates 3, 5, 9 and 10.

## 6. Hardware checklist (`docs/TESTING.md`)
1. Paired-but-disconnected buds show their name and "Not connected"; Retry works.
2. Take buds out → Connected within ~6–10 s; L/R/Case battery shown; mode matches the buds.
3. Off / Transparency / ANC switch audibly; UI changes instantly and doesn't flick back.
4. ANC levels Smart, Max, Moderate, Mild, Adaptive each apply and read back.
5. Mode changed on the bud (long-press) shows in the UI within ~1 s.
6. Battery and charging bolt update when a bud goes into the case.
7. EQ presets (Balanced/Vocal/Bass) and custom EQ bands apply and persist after reopening.
8. Low latency toggle applies; a gesture-triggered change shows up.
9. Find earbuds plays, Stop stops, auto-stops after 30 s.
10. Gestures: change Left double tap to Next track and verify with a real tap; Right side too.
11. Buds into the case → "Not connected" (no stuck "Connecting…"); out again → reconnects without clicking.
12. Auto-reconnect from phone-only connection (Gate 9); disabled when the setting is off.
13. Sleep/wake recovers.
14. Shortcuts + HUD + conflict detection.
15. Remember settings: pick ANC Max, change to Transparency on the phone app, reconnect to PC → Max restored.
16. Launch at login → tray icon present, no flyout.
17. Developer tab: packet log, raw `0106`, scan, noise bits, multipoint hosts.
18. `budsctl` direct (app closed) and via IPC (app running).
19. Tray icon ring/dim/theme; tooltip.
20. DPI 100/150/200%, two monitors, taskbar at top/left: flyout and HUD placement.

## 7. Risks and fallbacks
| Risk | Fallback |
|---|---|
| Winsock GUID connect doesn't find the HeyMelody service on Buds 4 | Fallback channel 15 (Buds 4's "enco link" channel per upstream); Phase 9 `sdp.rs` gives the real channel |
| RFCOMM page doesn't bring audio back on Windows | Phase 9 KS one-shot reconnect (what Windows Settings "Connect" uses) |
| Tray click / blur race reopens or closes the flyout | 300 ms blur timestamp guard (Phase 4) |
| Native popup menu steals focus and hides flyout | `set_menu_open` guard; if still broken on a platform, use an in-flyout dropdown |
| WebView2 transparency/Acrylic artifacts on Windows 10 | Solid background tokens when the effect call fails |
| `Monitor::work_area` missing in the pinned Tauri | `MonitorFromPoint` + `GetMonitorInfoW` via `windows` crate |
| Linux Wayland: no global hotkeys / window positioning | `budsctl cycle` bound in desktop settings; centred flyout; notifications for HUD |
| `objc2-io-bluetooth` missing APIs | Swift shim via `swift-rs` reusing upstream code |
| WebView memory (~60–100 MB incl. WebView2 processes) | Settings window destroyed on close; HUD created lazily; flyout is the only resident webview |
