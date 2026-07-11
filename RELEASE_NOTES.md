# devclean v0.3.0

This release adds a native SwiftUI macOS menu bar experience without moving any deletion authority out of the Rust safety engine.

## macOS menu bar

- Shows free disk space and estimated reclaimable bytes from the menu bar.
- Previews and selects exact candidates with native SwiftUI controls.
- Provides settings for roots, age/size filters, and opt-in cache categories.
- Bundles a universal Rust helper inside `Devclean.app`.

## Safety and automation

- Adds repeatable `clean --only-path PATH` selection for machine clients.
- Performs a fresh Rust scan and aborts before deletion if any selected path is stale.
- Invokes the helper directly without a shell; GUI never enables tracked-file or Docker-system escape hatches.
- Adds Swift contract tests, macOS app CI, universal app packaging, checksums, SBOM, and provenance attestations.

## Installation

The crates.io package remains `devclean-cli`; it installs the `devclean` executable. The menu bar archive contains `Devclean.app` and requires macOS 13 or newer.

Download the archive for your platform, verify `SHA256SUMS`, and run `gh attestation verify <archive> -R tuanle96/devclean`.
