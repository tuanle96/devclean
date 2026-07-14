# Changelog

All notable changes to this project are documented here following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Updated dependencies: `toml` 1.1, `rusqlite` 0.39, `crossterm` 0.29, `clap_mangen` 0.3, `uuid` 1.23; Cargo-workspace detection now parses manifests as TOML documents (`toml::Table`), matching the toml 1.x API. rusqlite stays below 0.40, whose libsqlite3-sys needs Rust 1.95 and would break the crate's MSRV of 1.85.
- Release and CI workflows moved to `actions/attest-build-provenance` v4, `actions/attest-sbom` v4, and `actions/setup-node` v6.
- macOS modal confirmation cards adopt Liquid Glass on macOS 26 while keeping the material-and-stroke look on macOS 13–15; banners, chips, and rows deliberately stay material per the floating-layer guidance.

## [0.7.0] - 2026-07-14

### Added

- Default-root discovery now checks existing `Dev`, `Developer`, `Projects`, `Code`, `src`, workspace/repository, GitHub, Android Studio, and IntelliJ project conventions without scanning the entire home directory.
- The exact global-cache allowlist now includes Gradle caches/distributions on every platform and Xcode DerivedData on macOS.

### Changed

- Scan roots are canonicalized and nested or duplicate roots are collapsed before traversal.
- macOS Settings renders the automatic locations actually present on disk, labels custom-root override semantics, and describes the expanded opt-in cache group accurately.
- macOS rows now lead with the owning project name; the artifact type moves to the icon tooltip, path suffix, and VoiceOver label, and artifact age and hold expiry render as capsule chips beside the path.
- macOS project names resolve from scanner-recognized workspace roots first, and the parent-directory fallback skips one generic member folder (services, backend, packages, …) unless it sits directly under a container folder like ~/Dev.

### Security

- CoreSimulator, Android SDK/AVD, JetBrains Local History, Docker Desktop storage, and the Gradle user-home root remain outside the filesystem cleanup allowlist.

## [0.6.0] - 2026-07-13

### Added

- `devclean analyze` correlates a current read-only scan with aggregate local history to report stale categories, repeated growth, cleanup failures, and workspace concentration in table or JSON form.
- Cargo, npm-workspaces, and Nx detection groups scan candidates under their nearest monorepo root and exposes additive workspace summaries in scan reports.
- Global-cache discovery falls back to `go env GOMODCACHE` when `GOMODCACHE` is not set, accepting only absolute paths.

### Changed

- The macOS candidate lists are native Lists with keyboard row navigation, project names in row titles ("Rust target · VibeTG"), and content-sized viewports.
- macOS cleanup and safety-hold confirmations use horizontal Mac alert button rows with Cancel beside the primary action, drop the extra "Back" step, and move VoiceOver focus into the dialog when it opens.
- The macOS Clean button states the selected size ("Clean 38.77 GB…"), "Delete All" moves into the Holds summary menu away from the window edge, and the Holds summary shows when the oldest hold expires.
- macOS AI recommendations keep a single manual entry point beside Scan, the monitoring banner sits on a neutral material background, and the AI dialog uses a standard trailing button row.
- The macOS menu bar icon keeps one externaldrive silhouette across every state and pulses while busy on macOS 14+; tapping a background-scan notification activates DevCleaner.
- macOS scan-status copy drops scanner jargon, the disk capacity bar explains its remaining "other used" slice and reads as one VoiceOver element, and Settings labels use consistent capitalization with `SettingsLink` on macOS 14+.
- Split scanner classification, core CLI subcommands, and the macOS menu content into focused modules without changing cleanup authority or public commands.
- Version the local SQLite history schema with transactional `PRAGMA user_version` migrations; legacy unversioned databases upgrade in place and newer unsupported schemas are rejected.
- Replace the internal free-form SQL sort direction with a closed enum and derive repeated category-growth counts from path-free history snapshots.

## [0.5.0] - 2026-07-13

### Added

- `devclean clean --dry-run` renders the exact filtered plan without confirmation, Docker invocation, quarantine, or deletion; `clean --undo <ID>` restores one exact safety hold as a quarantine shorthand.
- Safe-default Python cleanup for `__pycache__`, project-local `.tox`/`.nox`, and `.venv`/`venv` directories directly backed by a Python dependency or project manifest.
- Read-only `devclean watch` mode backed by native filesystem events, configurable threshold/scan interval, best-effort desktop notifications, and a deterministic `--once` mode for launch agents and verification.
- Build-output detection for Gradle/Kotlin, CMake, and Zig projects plus custom absolute `GOMODCACHE` locations.
- Structured Docker disk-usage parsing and an injectable command boundary for unit tests.
- Optional on-device AI Insights use Apple's Foundation Models framework on supported Macs to explain compact review facts with typed output; the model receives no full paths and has no cleanup, approval, hold, restore, or purge tools.
- OpenAI-compatible AI Insights initially support DeepSeek V4 Flash with strict JSON output, HTTPS-only endpoints, synthetic contract verification, explicit remote-processing disclosure, and API keys stored in macOS Keychain.
- The macOS cleanup confirmation now offers an explicit `Delete Now` path with a second irreversible-action confirmation, and Settings can permanently delete one exact safety hold or all listed holds immediately through separately confirmed actions.
- `devclean quarantine purge --id <ID>` permanently deletes one exact safety hold before or after expiry for trusted native clients and automation.
- The macOS companion is now branded `DevCleaner`, ships a HIG-scaled disk-and-sparkle app icon, and packages as `DevCleaner.app` while preserving the existing bundle identifier and local data paths.
- The macOS menu bar app now registers with macOS Launch at Login on first launch, exposes an approval-aware Settings toggle, and remains user-disableable. The Homebrew Cask opens the app after installation so registration happens immediately.
- Scanner-owned Learning Mode rules for Xcode `DerivedData`, Gradle `.gradle`, and CocoaPods `Pods` directories, using the same exact-path approval and pre-deletion revalidation flow as SwiftPM `.build`. The Gradle rule refuses the global `~/.gradle`, which can hold credentials; CocoaPods requires both `Podfile` and `Podfile.lock` so exact dependency versions remain reproducible.
- JSONL reports stream `learning_observation` events and count them in the summary event.
- Scheduled fuzz coverage for the duration/size filter parsers and a strict `swift format lint` gate in CI.
- Read-only `devclean tui` candidate browser with project grouping, category capacity bars, checkbox selection, and command preview; it never performs deletion itself.
- Platform-native `devclean schedule install/list/remove` with mandatory cleanup authority, dry-run installation preview, and structured JSONL cleanup results.
- Aggregate local SQLite scan and cleanup history plus `devclean stats` table, JSON, and HTML reports; candidate paths are deliberately not persisted.
- Repository-first `.devclean.toml`, `devclean init`, validated shared-config fetch from a Git URL or local checkout, and marker-backed custom cleanup rules that are revalidated before deletion.
- A VS Code status-bar companion with read-only scan/report/preview commands and a separately confirmed cleanup command.

### Changed

- The user-facing Learning Mode is now named Observation & Approvals, and Safety Hold retention is configured independently so disabling observation never silently changes cleanup into immediate deletion.
- The macOS menu now separates Clean, Review, and Holds into task-focused sections, prioritizes held disk space when no safe candidates remain, and exposes restore or separately confirmed permanent-delete actions beside each safety hold.
- Opening Settings from the menu bar now activates DevCleaner and promotes the titled Settings window to the key/front window instead of leaving it behind the MenuBarExtra popover or another app.
- Busy cleanup states in the macOS menu now replace the destructive controls with an explicit Hold, refresh, scan, or restore activity label; candidate selection stays locked until the refreshed result is ready.
- Cleanup confirmation now renders inside the MenuBarExtra window, preventing the popover from dismissing the system dialog before its Hold/Clean action can receive a click.
- Candidate size measurement uses Rayon's indexed parallel iterator, preserving deterministic result order. Performance evidence is reported as repeated-run medians because filesystem cache and background I/O can dominate a single large-tree scan.
- Git repository-root discovery is cached across scan and cleanup validation, report category totals are deterministically ordered, and quarantine identifiers use UUID v4 values.
- The Rust binary entrypoint and scanner measurement implementation are split into focused modules.
- The Swift menu UI now splits candidate, review, safety-hold, confirmation-state, and app-state components into focused source files.
- HTML report rendering now uses autoescaped MiniJinja templates instead of inline string concatenation.
- Quarantine expiry is printed as an RFC 3339 timestamp instead of raw Unix seconds.
- Terminal tables align the category column correctly.
- Default scan roots are `~/Dev` and `~/Projects`; configure additional roots in `devclean.toml`.
- The unknown scanner rules of a newer bundled helper no longer fail report decoding in the menu bar app, and learning state written by a newer app version keeps its known entries instead of resetting.
- Replaced the unmaintained `fs2` dependency with `fs4`.

### Documentation

- Clarified that `scan` always includes every rebuildable filesystem category and `--all` is accepted only for symmetry with `clean`.
- Documented the deliberate absence of the App Sandbox in the menu bar app.

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

[Unreleased]: https://github.com/tuanle96/devclean/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/tuanle96/devclean/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/tuanle96/devclean/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/tuanle96/devclean/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/tuanle96/devclean/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/tuanle96/devclean/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/tuanle96/devclean/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/tuanle96/devclean/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/tuanle96/devclean/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tuanle96/devclean/releases/tag/v0.1.0
