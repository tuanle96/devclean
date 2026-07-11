# Architecture

`devclean` separates configuration, discovery, policy, deletion, external tools, and rendering so safety decisions remain reviewable and testable.

## Modules

- `config.rs`: strict TOML loading and human duration/size parsing.
- `model.rs`: serializable categories, candidates, reports, and render options.
- `policy.rs`: compiled excludes and the Git tracked-file guard.
- `scanner.rs`: bounded traversal, classification, age/size filtering, platform cache allowlists, and allocated-size measurement.
- `cleaner.rs`: containment/category/Git revalidation and quarantine-based deletion.
- `docker.rs`: detailed usage, build-cache prune, and volume-preserving system prune.
- `render.rs`: terminal, JSON, JSONL, redacted, and standalone HTML reports.
- `main.rs`: CLI orchestration, candidate selection, target-free planning, confirmation, completions, and manpage generation.

## Cleanup lifecycle

```text
CLI + config
  -> bounded read-only traversal
  -> evidence-based classification
  -> exclude + age + size + Git guards
  -> exact cleanup plan
  -> optional target-free / interactive selection
  -> user confirmation
  -> containment + category + Git revalidation
  -> atomic same-parent quarantine rename
  -> recursive quarantine deletion
  -> post-clean verification
```

The scan report is not permanent deletion authority. Cleanup repeats live checks. Renaming the final path before recursive deletion narrows the time-of-check/time-of-use window and ensures a path swapped to a symlink is quarantined and rejected rather than followed.

## Size and age accounting

On Unix, allocated blocks are counted and hard-linked inodes are deduplicated. Other platforms use logical file length. The newest modification timestamp observed in the candidate tree drives `--older-than`. Traversal does not follow symlinks or cross the starting filesystem.

`--target-free` reads free space from the first scan root and selects size-sorted candidates until the estimated deficit is covered. It is a planning aid; APFS snapshots, compression, sparse files, and multi-filesystem roots can make actual reclaimed space differ.

## Adding a category

1. Define a narrow evidence rule in `scanner.rs`.
2. Decide whether the category belongs in conservative defaults.
3. Add positive, lookalike, protected-path, Git-tracked, and symlink tests.
4. Decide whether redownload/rebuild cost requires a separate opt-in.
5. Update README, safety documentation, skill policy, and changelog.
