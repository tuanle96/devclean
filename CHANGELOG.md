# Changelog

All notable changes to this project are documented here following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/tuanle96/devclean/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/tuanle96/devclean/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tuanle96/devclean/releases/tag/v0.1.0
