# devclean v0.8.0

This release adopts Liquid Glass in the DevCleaner menu bar app and lands a fully tested dependency refresh.

## Liquid Glass on macOS 26

- The three floating confirmation cards (cleanup, hold purge, AI recommendations) render as Liquid Glass on macOS 26 and keep their material-and-stroke look on macOS 13–15.
- Standard controls already pick up the new appearance from the macOS 26 SDK build; banners, chips, and rows deliberately stay material per the floating-layer guidance.

## Dependency refresh

- `toml` 1.1, `rusqlite` 0.39, `crossterm` 0.29, `clap_mangen` 0.3, `uuid` 1.23.5, plus `actions/attest-build-provenance` v4, `actions/attest-sbom` v4, and `actions/setup-node` v6 in CI.
- Cargo-workspace detection now parses manifests as TOML documents (`toml::Table`), matching the toml 1.x API — a unit test caught the silent behavior change.
- rusqlite stays below 0.40 on purpose: its libsqlite3-sys needs Rust 1.95 and would break the crate's MSRV of 1.85. Dependabot now documents and skips that upgrade until the MSRV policy moves.

## Verification

Release gates cover Rust formatting, Clippy with warnings denied, MSRV 1.85, and cross-platform test suites; Swift strict format linting and the full menu bar contract test suite; and a signed, notarized, stapled universal macOS app published with checksums, SBOM, and provenance attestations.
