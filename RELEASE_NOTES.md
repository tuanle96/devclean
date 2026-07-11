# devclean v0.2.0

This release turns the initial safety-first cleaner into a configurable, selective, distribution-ready tool.

## Safety

- Blocks Git-tracked candidates and repeats the guard immediately before cleanup.
- Atomically quarantines candidates before recursive deletion.
- Protects backup, database, and volume paths case-insensitively.
- Separates large model/runtime caches behind `--expensive-caches`.
- Makes `--docker` build-cache-only; broader cleanup requires `--docker-system`. Volumes remain untouched.

## Control and automation

- TOML config, exclude globs, age/size filters, target-free planning, and interactive range selection.
- JSONL and redacted JSON/HTML reports.
- Platform-aware cache paths, shell completions, and roff manpage generation.
- Integration/property tests, immutable Action pins, checksums, CycloneDX SBOM, and build/SBOM attestations.

## Installation note

The crates.io package is `devclean-cli`; it installs the `devclean` executable.

Download the archive for your platform, verify `SHA256SUMS`, and run `gh attestation verify <archive> -R tuanle96/devclean`.
