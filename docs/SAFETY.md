# Safety model

`devclean` assumes names can be misleading, repositories can intentionally track generated output, and filesystem state can change between scan and deletion.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Unrelated directory named `target` | Require Cargo build markers |
| Generated-looking output is committed | Block candidate when `git ls-files` finds tracked content |
| Symlink redirects deletion | Never follow links; reject symlink candidates before and after quarantine |
| Candidate changes after scan | Reclassify and repeat Git/containment checks immediately before deletion |
| Validation/removal race | Atomically rename to a unique same-parent quarantine, inspect, then remove |
| Candidate escapes selected root | Canonical containment check |
| Mounted storage is traversed | Stay on the starting filesystem |
| Hard links inflate estimates | Deduplicate Unix device/inode pairs |
| Generic output is a deliverable | Keep ambiguous `dist`, `out`, and `coverage` unclassified |
| Sensitive path differs only by case | Protect backup/database/volume names case-insensitively |
| Docker cleanup destroys persistent state | Never invoke prune with `--volumes`; default Docker mode is build cache only |
| Global cache rule expands unexpectedly | Exact, platform-aware allowlists; expensive model/runtime caches are separate |
| Shared report leaks workstation paths | `--redact-paths` replaces roots/home with placeholders |
| GUI selection becomes stale | `--only-path` requires every selected path to appear in a fresh Rust scan or aborts before deletion |
| GUI command injection | Swift launches the bundled helper directly with an argument array; no shell is involved |

## Deliberate escape hatches

- `--yes` removes the final interactive confirmation.
- `--allow-tracked` permits deletion of candidates containing Git-tracked files.
- `--expensive-caches` includes model and runtime downloads.
- `--docker-system` removes stopped containers and unused images/networks in addition to build cache.

The macOS app exposes none of these escape hatches. Its default categories are Rust targets, `node_modules`, and framework caches, filtered to artifacts older than 7 days and at least 100 MiB.

Treat these as explicit policy changes, not convenience defaults.

## Limitations

- Quarantine reduces path-replacement risk but is not a complete defense against a hostile process mutating files inside an already-open directory tree.
- Allocated bytes and target-free planning are estimates, especially on APFS, compressed, sparse, snapshot, and container-backed storage.
- Directory modification time is derived from the newest timestamp observed during size traversal; a process can regenerate data immediately afterward.
- Multi-root target-free planning uses the first root's filesystem.
- `--allow-tracked` can delete committed files; Git may restore them, but uncommitted changes inside the candidate may be lost.

Use `scan`, review HTML/JSON evidence, and rerun the scan plus `df` after cleanup.
