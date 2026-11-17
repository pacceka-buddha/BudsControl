# Releasing

Releases are built by GitHub Actions ([`.github/workflows/release.yml`](../.github/workflows/release.yml)) when a tag
starting with `v` is pushed. It builds the Windows installer and portable zip, the Linux AppImage and .deb, and the
macOS .dmg files (arm64 and x64), and attaches them to a GitHub release with generated notes.

## Steps

1. Make sure `main` is green on CI.
2. Set the new version in all of these (they must match):
   - `Cargo.toml` (`[workspace.package] version`)
   - `package.json` (`version`), then run `npm install` so `package-lock.json` follows
   - `src-tauri/tauri.conf.json` (`version`)
3. Run `cargo build --workspace` so `Cargo.lock` picks up the new version.
4. Move the `Unreleased` notes in [`CHANGELOG.md`](../CHANGELOG.md) under the new version and date.
5. Commit, tag and push:

   ```bash
   git commit -am "release v0.2.0"
   git tag v0.2.0
   git push origin main v0.2.0
   ```

6. Watch the **Release** workflow in the Actions tab. When it finishes, open the release, check the attached files,
   and edit the notes if needed.

## What a release should contain

Six files, named `BudsControl-<version>-<platform>.<ext>`. Sizes for 1.0.0, to spot a broken build at a glance
(a file far from these sizes means something was left out or bundled twice):

| File | About |
|---|---|
| `windows-x64-setup.exe` | 1.8 MiB |
| `windows-x64-portable.zip` | 2.5 MiB, holds `BudsControl.exe` and `budsctl.exe` |
| `linux-x64.AppImage` | 88 MiB |
| `linux-x64.deb` | 4.0 MiB |
| `macos-arm64.dmg` | 3.2 MiB |
| `macos-x64.dmg` | not measured yet (Intel build) |

Build the Windows package with `scripts/package-windows.ps1` (or `npm run tauri build -- --config
src-tauri/tauri.bundle.conf.json`). A plain `tauri build` leaves `budsctl` out of the installer. The full list of
figures is in [PERFORMANCE.md](PERFORMANCE.md).

## First release on a new repository

- In the repository's **Settings > Actions > General**, make sure workflows are allowed and that
  *Workflow permissions* is set to **Read and write** (the release job needs `contents: write`; the workflow asks for
  it, but an organization policy can override it).
- The builds are unsigned. Windows SmartScreen and macOS Gatekeeper will warn users; the README explains how to get
  past that. Code signing needs a certificate (Windows) or an Apple Developer ID (macOS) and is not set up.

## Building locally

The same packages can be built on each OS with the scripts in `scripts/` (output goes to `release/`). See
[Building from source](../README.md#building-from-source).
