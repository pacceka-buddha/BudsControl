# Security policy

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Use GitHub's private reporting instead: open the
repository's **Security** tab and choose **Report a vulnerability**.

Include what you found, how to reproduce it, and which version and OS you used. You'll get a reply as soon as
possible; this is a volunteer project, so please allow some time.

## Scope

Buds Control runs locally and talks only to your paired earbuds over Bluetooth. It has no network features and no
updater. Areas where a report is especially welcome:

- The local IPC channel between `budsctl` and the running app (a named pipe on Windows, a Unix socket on Linux and
  macOS).
- Parsing of data received from the earbuds.
- The installers and packaging scripts.

## Supported versions

Only the latest release receives fixes.
