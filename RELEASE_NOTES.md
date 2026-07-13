# devclean v0.7.0

This release makes the scanner meet developers where their code actually lives, and names every menu bar row after the project it belongs to.

## Smarter scan roots

- Default-root discovery now checks existing `Dev`, `Developer`, `Projects`, `Code`, `src`, workspace/repository, GitHub, Android Studio, and IntelliJ project conventions — without scanning the entire home directory.
- Scan roots are canonicalized, and nested or duplicate roots collapse before traversal.
- The exact global-cache allowlist adds Gradle caches/distributions on every platform and Xcode DerivedData on macOS.
- macOS Settings shows the automatic locations actually present on disk and labels custom-root override semantics.

## Project-first menu bar rows

- Every candidate, review, and hold row now leads with the owning project ("fastsoft-tg"), resolved from scanner-recognized workspace roots before falling back to the parent directory, which skips generic member folders like `services/` or `backend/`.
- The artifact type moves to the icon tooltip, path suffix, and VoiceOver label; artifact age and hold expiry render as capsule chips beside the path, so titles never crowd or truncate.

## Safety

- CoreSimulator, Android SDK/AVD, JetBrains Local History, Docker Desktop storage, and the Gradle user-home root remain outside the filesystem cleanup allowlist.

## Verification

Release gates cover Rust formatting, Clippy with warnings denied, and cross-platform test suites; Swift strict format linting and the full menu bar contract test suite; and a signed, notarized, stapled universal macOS app published with checksums, SBOM, and provenance attestations.
