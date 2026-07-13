# DevCleaner for macOS

`DevCleaner.app` is a native SwiftUI menu bar client for the Rust safety engine. It scans at launch and every six hours, records 30 days of local growth observations, keeps ambiguous discoveries review-only, and lets users approve or revoke exact paths only through scanner-owned rules. AI Insights can use Apple's on-device Foundation Models framework on macOS 26 or DeepSeek V4 Flash through its OpenAI-compatible API; both receive compact review facts without full paths or cleanup tools. Remote API keys live in macOS Keychain. Cleanup offers a restorable safety hold or a separately confirmed `Delete Now` path for immediate reclamation. Settings can restore or permanently delete one exact hold. Swift never deletes files itself and never invokes a shell.

The Homebrew Cask opens the app after installation. The first app launch registers `DevCleaner.app` with `SMAppService.mainApp`, so macOS opens it on subsequent logins. Users can disable this in DevCleaner Settings or System Settings > General > Login Items; explicitly quitting the app keeps it closed until the next login or manual launch.

```bash
brew install --cask tuanle96/tap/devclean-menubar
```

## Requirements

- macOS 13 or newer
- Swift 6 / Xcode 16 or newer for source builds
- Apple On-Device AI requires macOS 26, a compatible Mac, and Apple Intelligence enabled. DeepSeek works on the app's macOS 13 baseline with a separately configured API key and sends compact review facts to DeepSeek's remote service. All deterministic scan, approval, and cleanup features continue to work without either provider.

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
open dist/DevCleaner.app
```

Production distribution sets `CODE_SIGN_IDENTITY` to a Developer ID Application certificate, enables hardened runtime, notarizes the resulting archive, staples the ticket, and verifies Gatekeeper acceptance. The app intentionally does not enable App Sandbox because users select development roots across their home directory; macOS privacy protections still apply.

## App icon

The opaque 1024×1024 master is `Resources/AppIcon-master.png`. Regenerate the ten HIG sizes and `AppIcon.icns` after changing it:

```bash
apps/macos/scripts/generate-app-icon.swift
```

The build copies `AppIcon.icns` into `DevCleaner.app/Contents/Resources` and declares it through `CFBundleIconFile`.

Local state:

- `~/Library/Application Support/Devclean/learning.json` — private 0600 growth history and feedback.
- `~/Library/Application Support/devclean/quarantine.json` — private 0600 Rust safety-hold registry.
- `~/Library/Logs/Devclean/devclean.jsonl` — rotating private structured diagnostics.

The `Devclean` directory names and `com.tuanle.devclean.menubar` bundle identifier are intentionally retained so the rename to DevCleaner does not discard existing learning history, preferences, logs, or Launch at Login state.
