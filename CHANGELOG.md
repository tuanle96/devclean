# Changelog

All notable changes to this project are documented here following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.1] - 2026-07-11

### Added

- Review-to-safe approval flow for SwiftPM `.build` directories directly beside `Package.swift`.
- Exact-path learned approvals with visible approve/revoke controls in the macOS menu bar app.
- Approved observations remain visible while waiting for the configured age and size thresholds.

### Security

- Approval never accepts an arbitrary directory: Rust owns the rule, canonicalizes the approved path, repeats symlink/containment/Git checks, and revalidates the Swift package marker immediately before cleanup.
- Revoking an approval removes the path from cleanup selection and returns it to review-only state.

## [0.4.0] - 2026-07-11

### Added

- Learning Mode observations for active known artifacts and large unknown cache-like project directories.
- Thirty-day local growth history, recreation detection, and per-path `always-clean` / `never-clean` feedback.
- Restorable cleanup safety holds with list, restore, expiry purge, and a locked private registry.
- Six-hour background observation cycle in the native macOS menu bar app.
- Structured rotating local diagnostics plus a protocol-based, consent-gated Sentry provider.
- Privacy and observability contracts documenting local state, remote data boundaries, and event schemas.

### Security

- Review-only observations are structurally separate from cleanable candidates and cannot grant deletion authority.
- Learning/log registry files use mode 0600; quarantine restore and purge reject non-adjacent or symlink paths.
- Remote diagnostics default off, require both DSN and explicit consent, disable default PII, and never receive raw paths or project names from product events.

### Changed

- The menu bar app scans at launch and every six hours rather than waiting for the popover to open.
- Learning measurements ignore cleanup age/category filters while cleanup eligibility remains unchanged.

## [0.3.1] - 2026-07-11

### Security

- Sign the universal macOS app and bundled Rust helper with Developer ID Application and hardened runtime.
- Notarize every release with Apple's notary service, staple the ticket, and require Gatekeeper acceptance before publishing.
- Import signing material through an ephemeral CI keychain and remove it after the build.

### Changed

- Make the macOS release archive suitable for direct installation and a checksum-pinned Homebrew Cask.

## [0.3.0] - 2026-07-11

### Added

- Native SwiftUI macOS menu bar app with free-space status, scan preview, per-candidate selection, settings, confirmation, and post-clean refresh.
- Universal macOS app packaging with the Rust helper bundled inside `Devclean.app`.
- Exact `--only-path` cleanup selection for trusted machine clients and automation.
- Swift contract tests plus macOS app build and signature verification in CI.

### Security

- Menu bar cleanup always performs a fresh Rust scan and rejects the entire operation if any selected path is no longer eligible.
- The Swift client invokes the helper directly without a shell and never enables `--allow-tracked` or Docker system cleanup.

## [0.2.0] - 2026-07-11

### Added

- Git tracked-file protection at scan and clean time.
- Strict TOML configuration, exclude globs, age/size filters, target-free planning, and interactive candidate selection.
- JSONL output, path redaction, shell completions, and manpage generation.
- Separate cheap and expensive global-cache categories with platform-aware paths.
- Docker build-cache-only cleanup, explicit system cleanup, and age filters.
- Integration and property tests for destructive boundaries.
- Release artifact/SBOM attestations, a CycloneDX SBOM, and immutable GitHub Action pins.

### Changed

- Candidates are atomically quarantined before recursive deletion.
- Protected backup/database/volume path names are matched case-insensitively.
- The crates.io package name is `devclean-cli`; the executable remains `devclean`.

## [0.1.0] - 2026-07-11

### Added

- Read-only scanning with terminal, JSON, and HTML output.
- Conservative and comprehensive cleanup profiles.
- Evidence-based artifact detection, global cache cleanup, Docker cleanup, and companion Codex skill.

[Unreleased]: https://github.com/tuanle96/devclean/compare/v0.4.1...HEAD
[0.4.1]: https://github.com/tuanle96/devclean/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/tuanle96/devclean/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/tuanle96/devclean/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/tuanle96/devclean/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/tuanle96/devclean/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tuanle96/devclean/releases/tag/v0.1.0
