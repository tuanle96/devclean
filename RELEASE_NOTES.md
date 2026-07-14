# devclean v0.9.0

This release gives the DevCleaner menu bar app a read-only Memory tab for the dev tooling holding RAM, and makes launch instant by restoring the last scan from a local cache.

## Memory tab (read-only)

- A fourth tab reports the kernel's own memory-pressure level, an active+wired+compressed usage bar, and the restartable dev tooling currently holding RAM: JVM daemons, JavaScript runtimes, simulators, container VMs, language servers, and local databases.
- Rows lead with the owning project — derived from each process's working directory — and a chip naming what the runtime actually executes (`vite.js`, `Gradle daemon`), so twenty identical `node` processes stay tellable apart. Hovering a row reveals the PID and full working directory.
- Footprint is `ri_phys_footprint`, the figure Activity Monitor shows, pinned to `RUSAGE_INFO_V4` so a future SDK bump cannot silently blank the tab on older kernels. File cache is deliberately excluded from "used": counting reclaimable cache is how RAM cleaners invent work.
- Strictly read-only by design: sampling runs only while the menu is open, and nothing is signaled, killed, or purged. Display stays in Swift; any future termination authority belongs in the Rust CLI with its own revalidation.

## Instant launch from cached scans

- The last successful scan report persists to `~/Library/Application Support/Devclean/last-scan.json` and is restored at launch, so reopening the app shows the previous results immediately instead of blocking every tab behind the first scan.
- A background refresh still runs whenever the cache is older than 30 minutes, and a manual tab choice is no longer overridden when that scan completes.
- Cleanup safety is unchanged: the CLI revalidates every exact path before deletion, so a stale cached candidate can never be deleted.

## Verification

Release gates cover Rust formatting, Clippy with warnings denied, MSRV 1.85, and cross-platform test suites; Swift strict format linting and the menu bar contract test suite, including new coverage for process classification, argv detail derivation, working-directory project naming, and the report-cache round-trip; and a signed, notarized, stapled universal macOS app published with checksums, SBOM, and provenance attestations.
