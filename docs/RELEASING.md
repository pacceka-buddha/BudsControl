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

## First release on a new repository

- In the repository's **Settings > Actions > General**, make sure workflows are allowed and that
  *Workflow permissions* is set to **Read and write** (the release job needs `contents: write`; the workflow asks for
  it, but an organization policy can override it).
- The builds are unsigned. Windows SmartScreen and macOS Gatekeeper will warn users; the README explains how to get
  past that. Code signing needs a certificate (Windows) or an Apple Developer ID (macOS) and is not set up.

## Building locally

The same packages can be built on each OS with the scripts in `scripts/` (output goes to `release/`). See
[Building from source](../README.md#building-from-source).
