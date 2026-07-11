# Cleanup policy

## Category matrix

| Category | Default clean | Comprehensive clean | Evidence |
|---|---:|---:|---|
| Rust target | Yes | Yes | Directory is named `target` and contains Cargo build markers |
| node_modules | Yes | Yes | Exact `node_modules` or `frontend_node_modules` directory |
| Framework cache | Yes | Yes | Exact known cache name such as `.next` or `.svelte-kit` |
| Build output | No | Yes | `build` directory under a recognized project manifest |
| Test cache | No | Yes | Exact mutation, test, lint, or type-checker cache name |
| Global cache | No | Yes, opt-in | Exact path on the CLI allowlist |
| Docker image/build cache | No | Opt-in | `docker system prune -af`, never volumes |

## Always protected

- Docker and container volumes
- PostgreSQL, MySQL, SQLite, Odoo filestore, and other database data
- Backup folders and archive files
- Git, Mercurial, and Subversion metadata
- Environment and secret files
- User documents, downloads, media, and messaging data
- Ambiguous generated-looking paths that lack classification evidence

## Escalation cases

Stop and ask for a path-specific decision when:

- a large directory is tracked by Git;
- a generic `dist`, `out`, `coverage`, or `build` directory may be a deliverable;
- cleanup would stop a running process;
- a Docker volume appears unused but may hold a database;
- a backup or production-data copy is the largest reclaim opportunity;
- the filesystem cannot provide reliable allocated-size metadata.

## Regeneration handling

When a deleted artifact reappears:

1. Record its size and modification time.
2. Inspect working directories and open files with `lsof`.
3. Sample short-lived processes for Cargo, Rust, Node, Tauri, Vite, and package-manager commands.
4. Do not kill IDEs, agents, or watchers without user authority.
5. Retry only after the producer exits or the user authorizes stopping it.
