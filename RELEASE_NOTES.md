# devclean v0.4.0

This release turns the native menu bar app into a local learning and observability loop while preserving Rust as the only cleanup authority.

## Learning Mode

- Scans at launch and every six hours.
- Measures active known artifacts independently of cleanup age/category filters.
- Surfaces large unknown cache-like directories as review-only observations that cannot be cleaned.
- Keeps up to 30 days and 256 local snapshots, detects recreation after cleanup, and learns `Always select` / `Never clean` feedback.

## Restorable safety holds

- `clean --quarantine-for 7d` moves validated artifacts into hidden adjacent holds.
- `quarantine list`, `restore`, and `purge` manage the private registry.
- The UI states explicitly that holds retain disk usage until purge and refuses to overwrite a recreated original path.

## Diagnostics and privacy

- Rotating structured local JSONL logs are always available from Settings.
- The protocol-based monitoring layer ships with NoOp/local/Sentry providers.
- Sentry is disabled without a configured DSN and explicit user consent.
- Remote product events contain aggregate buckets and error fingerprints only; paths, usernames, project names, file contents, screenshots, and view hierarchy are excluded.

## Verification

Release gates cover Rust unit/integration/doc tests, Swift contract/learning/log tests, Clippy, MSRV, package verification, universal app build, Developer ID signing, Apple notarization, stapling, Gatekeeper acceptance, checksums, SBOM, and provenance attestations.
