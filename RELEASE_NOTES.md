# devclean v0.6.0

This release teaches the CLI to explain disk history — not just measure it — and completes the Apple HIG pass for the DevCleaner menu bar app.

## Analysis and workspaces

- `devclean analyze` correlates a current read-only scan with aggregate local history to report stale categories, repeated growth, cleanup failures, and workspace concentration in table or JSON form. It never deletes anything.
- Cargo, npm-workspaces, and Nx detection groups scan candidates under their nearest monorepo root and exposes additive workspace summaries in scan reports.
- Global-cache discovery falls back to `go env GOMODCACHE` when `GOMODCACHE` is not set, accepting only absolute paths.
- The local SQLite history schema is versioned with transactional migrations; legacy unversioned databases upgrade in place and newer unsupported schemas are rejected instead of misread.

## macOS menu bar app

- Candidate lists are native Lists with keyboard row navigation, project names in row titles ("Rust target · VibeTG"), and content-sized viewports.
- Cleanup and safety-hold confirmations use horizontal Mac alert button rows with Cancel beside the primary action, drop the extra "Back" step, and move VoiceOver focus into each dialog as it opens.
- The Clean button states the selected size ("Clean 38.77 GB…"), "Delete All" moves into the Holds summary menu away from the window edge, and the Holds summary shows when the next hold expires.
- AI recommendations keep a single manual entry point beside Scan, and the monitoring banner sits on a neutral material background that respects the system accent color.
- The menu bar icon keeps one externaldrive silhouette across every state and pulses while busy on macOS 14+; tapping a background-scan notification activates DevCleaner.
- Scan-status copy drops scanner jargon, the disk capacity bar explains its remaining "other used" slice and reads as one VoiceOver element, and Settings labels use consistent capitalization.

## Verification

Release gates cover Rust formatting, Clippy with warnings denied, and cross-platform test suites; Swift strict format linting and the full menu bar contract test suite; and a signed, notarized, stapled universal macOS app published with checksums, SBOM, and provenance attestations.
