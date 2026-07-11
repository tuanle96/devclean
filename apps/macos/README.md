# Devclean for macOS

`Devclean.app` is a native SwiftUI menu bar client for the Rust safety engine. It scans at launch and every six hours, records 30 days of local growth observations, keeps ambiguous discoveries review-only, and manages restorable safety holds. It never deletes files itself and never invokes a shell.

## Requirements

- macOS 13 or newer
- Swift 6 / Xcode 16 or newer for source builds

## Develop

```bash
cargo build
DEVCLEAN_EXECUTABLE="$PWD/target/debug/devclean" \
  swift run --package-path apps/macos DevcleanMenuBar
```

Run tests:

```bash
swift test --package-path apps/macos
```

Set `DEVCLEAN_SENTRY_DSN` for an opt-in development monitoring build. Without a DSN, local JSONL diagnostics remain active and the remote sharing toggle is disabled.

Build an ad-hoc signed local app bundle:

```bash
apps/macos/scripts/build-app.sh
open dist/Devclean.app
```

Production distribution sets `CODE_SIGN_IDENTITY` to a Developer ID Application certificate, enables hardened runtime, notarizes the resulting archive, staples the ticket, and verifies Gatekeeper acceptance. The app intentionally does not enable App Sandbox because users select development roots across their home directory; macOS privacy protections still apply.

Local state:

- `~/Library/Application Support/Devclean/learning.json` — private 0600 growth history and feedback.
- `~/Library/Application Support/devclean/quarantine.json` — private 0600 Rust safety-hold registry.
- `~/Library/Logs/Devclean/devclean.jsonl` — rotating private structured diagnostics.
