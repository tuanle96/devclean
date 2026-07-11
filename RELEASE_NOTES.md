# devclean v0.3.1

This patch makes the native macOS menu bar app ready for frictionless public distribution while preserving the v0.3 safety model.

## Trusted macOS distribution

- Signs the universal app and bundled Rust helper with Developer ID Application.
- Enables hardened runtime and Apple secure timestamps.
- Requires Apple notarization acceptance and staples the ticket before packaging.
- Verifies strict code signatures and Gatekeeper acceptance in the release workflow.

## Safety

- Swift remains an unprivileged presentation client; Rust still performs all scan, revalidation, quarantine, and deletion work.
- Exact-path cleanup still aborts before deletion if any selected candidate is stale.
- Signing credentials live only in GitHub encrypted secrets and an ephemeral CI keychain.

## Installation

The crates.io package remains `devclean-cli`; it installs the `devclean` executable. The notarized universal menu bar archive contains `Devclean.app` and requires macOS 13 or newer.

Download the archive for your platform, verify `SHA256SUMS`, and run `gh attestation verify <archive> -R tuanle96/devclean`.
