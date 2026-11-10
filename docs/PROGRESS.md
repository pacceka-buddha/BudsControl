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
