# HeyMelody earbuds protocol

What the OnePlus / OPPO / realme earbuds speak on their control channel. Derived from the original
[BudsControl](https://github.com/amarsakore/BudsControl) (`Sources/Protocol.swift`, `BudsController.swift`) and the
protocol references credited in `THIRD_PARTY_NOTICES.md`. The Rust implementation is `crates/buds-core`
(`protocol.rs`, `parse.rs`, `encode.rs`); tests are in `crates/buds-core/tests/gate1.rs`.

## Transport

Classic Bluetooth **RFCOMM**. The buds expose a "HeyMelody" control service ("enco link"), channel 15 on the Buds 4.
Two service UUIDs are used by different generations:

| UUID | Used by |
|---|---|
| `0000079A-D102-11E1-9B23-00025B00A5A5` | HeyMelody control service |
| `00001107-D102-11E1-9B23-00025B00A5A5` | HeyMelody control service (other generation) |
| `00001101-0000-1000-8000-00805F9B34FB` (SPP) | realme buds, only when no HeyMelody service exists |

If no service record is found, channels 15, 12, 13 are tried in that order.

## Frames

```
AA <len> 00 00 <cmd lo> <cmd hi> <seq> <plen lo> <plen hi> <payload...>
```

- `len` counts the bytes after it: `7 + payload length`. When **sending** it is one byte. When **receiving** it is a
  **LEB128 varint** (7 bits per byte, high bit set = more bytes), so long packets such as the multipoint device list
  can use two or more bytes. Example: a 200-byte payload has `len = 207` and is sent as `CF 01`.
- `cmd` and the payload length are 16-bit little-endian.
- `seq` is a wrapping byte the sender increments before each request; replies echo the request's `seq`.
- A reply uses the request command with the high bit set: `0x0106` -> `0x8106`.
- The first payload byte of most replies is a **status** (0 = success).

Parser rules (`FrameParser`): skip bytes until `AA`; read the varint; wait until the whole body has arrived; keep
an incomplete tail for the next chunk. Our parser additionally treats a varint longer than 4 bytes or a length above
65535 as line noise (the Swift app would wait on it forever).

Worked example: battery request, seq 1: `AA 07 00 00 06 01 01 00 00`.

## Commands

| Command | Purpose | Request payload | Reply / push payload |
|---|---|---|---|
| `0x0100` | hello | – | `<status> <capability bitmap, LSB first>`; bit index = `byteIndex*8 + bit` |
| `0x0103` | product ID | – | `<status 0> <b1> <b2> <b3>`; ID string = `b3 b2 b1` as hex (Buds 4 = `065414`) |
| `0x0106` | battery | – | `<status> <count> (<id> <level \| 0x80 if charging>)*`; id 1 left, 2 right, 3 case |
| `0x0109` | wear status | – | `<status> <count> (<side> <0 in case \| 1 out of ear \| 3 in ear>)*` |
| `0x010C` | noise mode | `01 01` | contains `01 01 <mask lo> [<mask hi>]`; lowest set bit is the active mode |
| `0x0112` | paired devices (multipoint) | – | `<status> <count> (<addr, reversed 6 bytes> <reserved> <2 = connected> <1 = this host> <name len> <name>)*` |
| `0x0205` | subscribe to pushes | `03 01 02 03` | – (first byte is the number of ids) |
| `0x0504` | EQ preset pushed by the buds | – | `<id>` when the preset changes (stem gesture, phone app) |
| `0x0204` | push from the buds | – | first byte = subscription type (below), rest laid out like the matching reply |
| `0x0108` | read gestures | – | `<status> <count> (<side> <context> <gesture> <action>)*` |
| `0x0401` | set gesture | `01 <side> 01 <gesture> <action>` | `<status>` |
| `0x010D` | read on/off features | `01 06` (count, ids) | `<status> <count> (<id> <0\|1>)*` |
| `0x0403` | set on/off feature | `<id> <0\|1>` (`06` = low latency) | `<status>`; the buds confirm with a push ~0.5 s later |
| `0x010F` | read EQ preset | – | `<status 0> <preset id>` |
| `0x0406` | set EQ preset | `<id>` | `<status>` |
| `0x0122` | list custom EQs | `01 05` | `<status> <count> (<selected> <min dB> <max dB> <id> <name len> <name> <band count> (<Hz lo> <Hz hi> <gain>)*)*` |
| `0x0418` | rewrite a custom EQ | `02 <min> <max> <id> <name len> <name> <band count> (<Hz lo> <Hz hi> <gain>)*` | `<status>` |
| `0x0400` | find my earbuds | `01` start / `00` stop | `<status>`; a stop is answered with 1, so only a start can fail |
| `0x0404` | set noise mode | `01 01 <mask>` | `<status>`; then re-read with `0x010C` |
| `0x040B` | multipoint action | – | `<status>` |

dB values and gains are signed bytes (`i8`). The device list and EQ names are UTF-8.

### Push types (first byte of `0x0204`)

| Type | Meaning | Payload layout |
|---|---|---|
| `0x01` | battery | like `0x0106` |
| `0x02` | wear | like `0x0109` |
| `0x03` | noise mode | like `0x010C` |
| `0x05` | low latency | `05 <0\|1>` |
| `0x06` | paired devices | like `0x0112` |
| other | – | the app just refreshes |

## Noise-mode bits

The mask is little-endian, `bit / 8 + 1` bytes long when *setting*. The meaning of each bit is **per model** and
differs between *setting* and *reporting*. Each bit stands for a HeyMelody "modeType":

| modeType | Meaning |
|---|---|
| 1 | Off |
| 2 | Transparency |
| 3 | ANC · Mild |
| 4 | ANC · Max |
| 5 | ANC (no level; models without levels) |
| 6 | Voice enhancement (shown as Transparency) |
| 7 | ANC · Smart |
| 8 | ANC · Moderate |
| 10 | ANC · Adaptive |

OnePlus Buds 4 (`065414`): bit 0 = Off, 1 = ANC, 2 = Transparency, 3 = Off, 4 = Max, 5 = Moderate, 6 = Mild,
7 = Smart, 8 = Transparency, 11 = Adaptive. When *setting*, the app sends the **lowest** bit that has the wanted type
(Off = bit 0, Transparency = bit 2), but the buds *report* Off as bit 3 and Transparency as bit 8, which is why the
reported layout is looked up separately. OnePlus Buds Pro 3 (`064014`) additionally reports its levels in bits 12-15
(12 = Max, 13 = Moderate, 14 = Mild, 15 = Smart). realme buds use
one shared layout (off, transparency, mild, max, moderate, smart in bits 0-5) and their own gesture codes
(assistant = `0x04`, no noise-control gesture). The per-model data is `crates/buds-core/src/catalog/generated.rs`.

## Gestures

Side: `1` left, `2` right. Context: `01` music (what the app edits), `06` during a call (ignored). Gesture codes:
`2` double tap, `3` triple tap, `4` touch & hold. Actions:

| Code | Action | Code | Action |
|---|---|---|---|
| `00` | None | `08` | Noise control |
| `01` | Play / Pause | `0B` | Volume up |
| `03` | Voice assistant | `0C` | Volume down |
| `04` | Voice assistant (realme) | `11` | Low latency |
| `05` | Previous track | | |
| `06` | Next track | | |

## Capability bits (reply to `0x0100`)

A feature is only shown when its bit is set (if the buds sent a bitmap), the catalog allows it (if the model is
known) and the buds answered the matching query.

| Bit | Feature | Commands |
|---|---|---|
| 3 | gestures | `0x0108` / `0x0401` |
| 5 | find my earbuds | `0x0400` |
| 7 | low latency | `0x0403` |
| 8 | noise control | `0x010C` / `0x0404` |
| 10 | equalizer | `0x010F` / `0x0406` |
| 34 | custom EQ | `0x0122` / `0x0418` |

## Connection sequence (after the channel opens)

`hello` -> `productID` -> (1.5 s later: send any mode requested while disconnected) -> `subscribe 03 01 02 03` ->
`wear` -> `multipoint` -> `EQ` -> `EQ list 01 05` -> `switch 01 06` -> `gestures` -> `battery` + `noise query`.
If the buds don't answer a request within 3 s the link is considered dead and is reopened.
