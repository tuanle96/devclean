# Cleanup policy

## Category matrix

| Category | Default | Explicit opt-in | Evidence |
|---|:---:|---|---|
| Rust target | Yes | — | Exact `target` plus Cargo build markers |
| node_modules | Yes | — | Exact dependency directory name |
| Framework cache | Yes | — | Exact known framework cache name |
| Build/test output | No | `--all` | Recognized manifest and exact generated name |
| Package/tool cache | No | `--global-caches` | Exact platform-aware allowlist |
| Model/runtime cache | No | `--expensive-caches` | Separate allowlist and high redownload cost |
| Docker build cache | No | `--docker` | Builder prune with optional age filter |
| Docker system data | No | `--docker-system` | Stopped containers and unused image/network/cache; never volumes |

## Invariants

- Block Git-tracked candidates unless `--allow-tracked` is explicitly authorized.
- Revalidate category, containment, type, and tracked state immediately before cleanup.
- Quarantine by same-parent rename before recursive deletion.
- Never follow symlinks or cross filesystem boundaries during discovery.
- Protect VCS, backup, database, filestore, and volume names case-insensitively.
- Redact paths in reports that may leave the workstation.

## Escalate instead of deleting

Stop for a path-specific decision when a candidate contains tracked changes, resembles a deliverable, is being regenerated, is a backup or database copy, requires `--allow-tracked`, or lies outside the exact cache allowlist.

## Regeneration handling

Record size and modification time, inspect working directories/open files with `ps` and `lsof`, identify Cargo/Node/Tauri/Vite/package-manager producers, and retry only after the producer exits or the user authorizes stopping it.
