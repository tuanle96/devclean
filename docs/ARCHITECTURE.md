# Architecture

`devclean` separates configuration, discovery, policy, deletion, external tools, and rendering so safety decisions remain reviewable and testable.

## Modules

- `config.rs`: strict TOML loading and human duration/size parsing.
- `model.rs`: serializable categories, candidates, reports, and render options.
- `policy.rs`: compiled excludes and the Git tracked-file guard.
- `scanner.rs` and `scanner/`: bounded traversal, evidence-based classification, age/size filtering, platform cache allowlists, and allocated-size measurement.
- `cleaner.rs`: containment/category/Git revalidation and quarantine-based deletion.
- `quarantine.rs`: locked, private registry for persistent safety holds, restore, and expiry purge.
- `docker.rs`: detailed usage, build-cache prune, and volume-preserving system prune.
- `render.rs`: terminal, JSON, JSONL, redacted, and standalone HTML reports.
- `history.rs`: versioned, aggregate-only SQLite scan and cleanup history with transactional migrations.
- `cli/`: focused scan, clean, quarantine, doctor, scheduler, watch, TUI, stats, config, completion, and manpage orchestration; `main.rs` only enters the CLI.
- `apps/macos`: SwiftUI `MenuBarExtra` with menu content split by modal, AI, cleanup-confirmation, section, and action concerns; six-hour background observations, local learning state, structured diagnostics, opt-in Sentry provider, and direct process execution of the bundled Rust helper.

## Cleanup lifecycle

```text
  CLI / macOS menu bar + config
  -> bounded read-only traversal
  -> evidence-based classification
  -> exclude + age + size + Git guards
  -> exact cleanup plan
  -> optional target-free / interactive / exact-path selection
  -> user confirmation
  -> containment + category + Git revalidation
  -> atomic same-parent quarantine rename
    -> recursive quarantine deletion
  -> post-clean verification
```

The scan report is not permanent deletion authority. Cleanup repeats live checks. Renaming the final path before recursive deletion narrows the time-of-check/time-of-use window and ensures a path swapped to a symlink is quarantined and rejected rather than followed.

With `--quarantine-for`, the same validated rename becomes a persistent adjacent safety hold recorded in a locked 0600 registry. Holds remain on the same filesystem and therefore do not reclaim space until `quarantine purge`. Restore refuses to overwrite a recreated original path.

The macOS app is an unprivileged presentation client. It parses `scan --format json`, lets the user select candidates, then invokes `clean --only-path ... --yes`. The Rust process repeats discovery and refuses the whole selection if any requested path is stale or no longer eligible. Swift never invokes a shell or performs filesystem deletion.

Learning Mode emits `learning_observations` independently of cleanup age/category filters. Known artifacts receive safe or protected confidence; unknown cache-like directories beneath recognized projects receive review confidence. Only `candidates` can be passed to cleanup. Swift stores at most 30 days/256 snapshots locally and sends only aggregate buckets to monitoring providers.

## Size and age accounting

On Unix, allocated blocks are counted and hard-linked inodes are deduplicated. Other platforms use logical file length. The newest modification timestamp observed in the candidate tree drives `--older-than`. Traversal does not follow symlinks or cross the starting filesystem.

`--target-free` reads free space from the first scan root and selects size-sorted candidates until the estimated deficit is covered. It is a planning aid; APFS snapshots, compression, sparse files, and multi-filesystem roots can make actual reclaimed space differ.

## Adding a category

1. Define a narrow evidence rule in `scanner/classifier.rs`.
2. Decide whether the category belongs in conservative defaults.
3. Add positive, lookalike, protected-path, Git-tracked, and symlink tests.
4. Decide whether redownload/rebuild cost requires a separate opt-in.
5. Update README, safety documentation, skill policy, and changelog.
