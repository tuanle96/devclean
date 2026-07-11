# Safety model

`devclean` assumes filesystem names can be misleading and filesystem state can change between scanning and deletion.

## Protected assets

The cleanup policy is designed to preserve source code, VCS metadata, backups, databases, Docker volumes, environment files, secrets, and user documents.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| Unrelated directory named `target` | Require Cargo build markers |
| Symlink redirects deletion | Do not follow or accept symlinks |
| Candidate changes after scan | Reclassify immediately before deletion |
| Candidate escapes selected root | Canonical containment check |
| Mounted storage counted or traversed | Stay on the starting filesystem |
| Hard links inflate estimates | Deduplicate Unix device/inode pairs |
| Generic output is actually a deliverable | Keep ambiguous `dist`, `out`, and `coverage` unclassified |
| Docker cleanup destroys database state | Never invoke prune with `--volumes` |
| Global cache rule expands unexpectedly | Use exact home-relative allowlist paths |

## Limitations

- Allocated-size reporting remains an estimate and may differ from APFS or container runtime accounting.
- A separate process can regenerate artifacts during or after cleanup.
- The CLI cannot determine whether an unused-looking archive or Docker volume is valuable; these assets remain out of scope.
- `--yes` is intended for deliberate automation and removes the interactive barrier.

Use `scan` and review HTML or JSON evidence before running cleanup in automation.
