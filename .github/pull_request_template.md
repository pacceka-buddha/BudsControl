## What and why

<!-- What does this change, and why? Link the issue if there is one. -->

## How it was tested

<!-- OS, earbud model (or simulated buds via BUDS_FAKE=1), and what you checked. -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
- [ ] `npm run check` and `npm test` pass
- [ ] Regenerated `src/lib/bindings/` is committed (if Rust types exported to TypeScript changed)
- [ ] README / docs updated (if behaviour changed)
