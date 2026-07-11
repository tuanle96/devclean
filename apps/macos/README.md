# Devclean for macOS

`Devclean.app` is a native SwiftUI menu bar client for the Rust safety engine. It never deletes files itself and never invokes a shell. The bundled `devclean` helper performs a fresh scan and full cleanup revalidation for every selected path.

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

Build an ad-hoc signed local app bundle:

```bash
apps/macos/scripts/build-app.sh
open dist/Devclean.app
```

Production distribution sets `CODE_SIGN_IDENTITY` to a Developer ID Application certificate, enables hardened runtime, notarizes the resulting archive, staples the ticket, and verifies Gatekeeper acceptance. The app intentionally does not enable App Sandbox because users select development roots across their home directory; macOS privacy protections still apply.
