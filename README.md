# Buds Control (cross-platform)

A tray app for switching **Noise Cancellation**, **Transparency** and **Off** on OnePlus, OPPO and realme earbuds
without the phone app. A Windows-first port (Windows, then Linux and macOS) of
[amarsakore/BudsControl](https://github.com/amarsakore/BudsControl), built with Rust + Tauri 2 and Svelte.

> **Status:** early development. See [PLAN.md](PLAN.md) for the phased plan and
> [docs/PROGRESS.md](docs/PROGRESS.md) for where it stands.

## Planned features

Same as the original macOS app: one-click noise control and ANC levels, battery in the tray icon, equalizer and
custom EQ, low latency, find my earbuds, touch gestures, global shortcuts with an on-screen HUD, automatic
reconnect, remembered settings, developer tools and a CLI (`budsctl`).

## Development

Needs Rust (stable, MSVC toolchain on Windows), Node 20+ and the Visual Studio C++ Build Tools.

```powershell
npm install
npm run check          # svelte-check + tsc
cargo build --workspace
npm run tauri dev      # opens the app (BUDS_FAKE=1 will use simulated buds from Phase 2)
```

The original macOS app is cloned to `reference/upstream/` (gitignored) at commit `298d452` for porting.

## Credits

- [amarsakore](https://github.com/amarsakore) and contributors of the original BudsControl.
- Protocol references: the Noctalia `oppo-pods` plugin (MIT), Gadgetbridge's OPPO support, and RawS-Music's
  HeyMelody port (Apache-2.0). See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Disclaimer

Unofficial project, not affiliated with or endorsed by OnePlus, OPPO, realme or Microsoft. Use at your own risk.

## License

[MIT](LICENSE)
