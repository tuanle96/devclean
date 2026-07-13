# devclean v0.5.0

This release expands DevCleaner from a safe cleanup CLI into a local developer-disk toolkit while keeping deletion authority narrow and explicit.

## New workflows

- Browse candidates in a read-only `devclean tui`; it prints an exact cleanup command but never deletes.
- Install, inspect, or remove a platform-native cleanup timer with `devclean schedule`; installation requires explicit `--yes` authority and supports a complete dry run.
- Track aggregate scan and cleanup trends in a local SQLite database and render them with `devclean stats`; candidate paths are never persisted.
- Share repository policy through `.devclean.toml`, `devclean init`, validated Git-backed config fetch, and marker-backed custom rules.
- Use the VS Code status-bar companion for workspace scans, HTML reports, dry-run previews, and separately confirmed cleanup.

## Ecosystem and automation

- Safe Python bytecode, test-environment, and project-local virtual-environment detection.
- Gradle/Kotlin, CMake, Zig, and custom Go module-cache support.
- Native filesystem watch mode with thresholds, scan intervals, and best-effort notifications.
- JSONL automation output for cleanup jobs.

## Safety and architecture

- `clean --dry-run` and `clean --undo` make preview and recovery first-class.
- Custom rules require exact directory names and direct project markers, are rejected on unsafe path components, and are revalidated immediately before deletion.
- HTML is rendered through autoescaped templates.
- Rust CLI/measurement commands and Swift candidate, review, hold, settings, and menu-state components are split into focused modules.

## Verification

Release gates cover Rust unit, integration, documentation, Clippy, MSRV, package, and live watch checks; Swift formatting, contract tests, app build, and codesign; and TypeScript build, tests, and type checking for the VS Code extension. Published macOS archives additionally require Developer ID signing, notarization, stapling, Gatekeeper acceptance, checksums, SBOM, and provenance attestations.
