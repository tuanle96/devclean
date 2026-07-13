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
| Learning heuristic promotes ambiguous data | Review observations are a distinct type and cannot enter cleanup selection |
| User approves an arbitrary observed path | Approval is accepted only for an exact canonical path matching a scanner-owned rule; cleanup revalidates that rule and Git guard |
| Learning rule matches the global `~/.gradle` credentials store | The Gradle rule rejects a `.gradle` directory directly inside the home directory |
| CocoaPods cleanup cannot reproduce exact dependency versions | Require both `Podfile` and `Podfile.lock` beside an approved `Pods` directory, at scan and cleanup time |
| A directory merely happens to be named `.venv` | Require the environment to be directly beside a Python project/dependency manifest; repeat classification before cleanup |
| Sensitive path differs only by case | Protect backup/database/volume names case-insensitively |
| Docker cleanup destroys persistent state | Never invoke prune with `--volumes`; default Docker mode is build cache only |
| Global cache rule expands unexpectedly | Exact, platform-aware allowlists; expensive model/runtime caches are separate |
| Shared report leaks workstation paths | `--redact-paths` replaces roots/home with placeholders |
| GUI selection becomes stale | `--only-path` requires every selected path to appear in a fresh Rust scan or aborts before deletion |
| GUI command injection | Swift launches the bundled helper directly with an argument array; no shell is involved |
| Persistent hold registry is tampered with | Purge/restore require an exact same-parent `.devclean-quarantine-*` directory and reject symlinks |
| Immediate purge targets unrelated holds | Native clients pass one exact hold ID; the helper validates that registry entry and deletes no other hold |
| Dry-run unexpectedly mutates state | `clean --dry-run` exits before confirmation, Docker invocation, quarantine, or deletion |
| Watch mode gains cleanup authority | `watch` only calls the read-only scanner; threshold crossings instruct the user to review with `scan` and never invoke `clean` |
| Telemetry leaks workstation identity | Remote sharing is opt-in, `sendDefaultPii` is false, paths/project names are excluded, screenshots/view hierarchy are unavailable on macOS |

## Deliberate escape hatches

- `--yes` removes the final interactive confirmation.
- `--allow-tracked` permits deletion of candidates containing Git-tracked files.
- `--expensive-caches` includes model and runtime downloads.
- `--docker-system` removes stopped containers and unused images/networks in addition to build cache.

The macOS app exposes none of these policy escape hatches. Its cleanup candidates remain filtered to artifacts older than 7 days and at least 100 MiB. Users may choose a restorable hold or a separately confirmed permanent `Delete Now`; both paths repeat the same exact-path Rust validation, while immediate purge of an existing hold requires its exact registry ID. Learning observations ignore the age filter so active growth is visible. An approval grants authority only to the exact path and scanner-owned rule shown in the UI; age, size, containment, symlink, manifest, and Git guards remain mandatory.

Treat these as explicit policy changes, not convenience defaults.

## Limitations

- The menu bar app does not enable the App Sandbox: its purpose is deleting arbitrary user-selected development directories, which the sandbox model cannot express. Distribution integrity relies on Developer ID signing and notarization, and every deletion is still delegated to the validated Rust helper.
- Quarantine reduces path-replacement risk but is not a complete defense against a hostile process mutating files inside an already-open directory tree.
- Allocated bytes and target-free planning are estimates, especially on APFS, compressed, sparse, snapshot, and container-backed storage.
- Directory modification time is derived from the newest timestamp observed during size traversal; a process can regenerate data immediately afterward.
- Multi-root target-free planning uses the first root's filesystem.
- `--allow-tracked` can delete committed files; Git may restore them, but uncommitted changes inside the candidate may be lost.
- Persistent safety holds intentionally continue using disk space until purge. They are a recovery mechanism, not immediate reclamation.
- Python environments can contain manually installed packages that are not declared by the adjacent manifest; review `clean --dry-run` output when environment recreation cost matters.

Use `scan`, review HTML/JSON evidence, and rerun the scan plus `df` after cleanup.
