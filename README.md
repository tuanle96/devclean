# devclean

[![CI](https://github.com/tuanle96/devclean/actions/workflows/ci.yml/badge.svg)](https://github.com/tuanle96/devclean/actions/workflows/ci.yml)
[![Security](https://github.com/tuanle96/devclean/actions/workflows/security.yml/badge.svg)](https://github.com/tuanle96/devclean/actions/workflows/security.yml)
[![Release](https://img.shields.io/github/v/release/tuanle96/devclean)](https://github.com/tuanle96/devclean/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-65d6ad.svg)](LICENSE)
[![MSRV: 1.85](https://img.shields.io/badge/MSRV-1.85-79b8ff.svg)](Cargo.toml)

`devclean` is a safety-first Rust CLI for auditing and removing rebuildable development artifacts. It reclaims space from Rust builds, JavaScript dependencies, framework caches, selected package-manager caches, and unused Docker data while protecting source code, backups, databases, and Docker volumes.

## Why devclean?

Developer machines quietly accumulate tens or hundreds of gigabytes in `target`, `node_modules`, framework build caches, package caches, and container layers. Broad cleanup commands can reclaim that space, but they can also remove databases, backups, or unrelated directories with misleading names.

`devclean` uses an audit-first workflow:

1. Scan and classify candidates using filesystem evidence.
2. Show an exact cleanup plan and estimated allocated size.
3. Require confirmation before deletion.
4. Revalidate every candidate immediately before removal.
5. Never pass `--volumes` to Docker cleanup.

## Features

- Read-only terminal, JSON, and standalone HTML reports.
- Conservative and comprehensive cleanup profiles.
- Rust `target` detection that requires Cargo build markers.
- Safe handling of symlinks, mount boundaries, hard links, and changed paths.
- Exact allowlist for global development caches.
- Optional Docker image, stopped-container, network, and build-cache cleanup.
- Codex skill for repeatable audit, approval, cleanup, and verification workflows.
- macOS, Linux, and Windows CI coverage.

## Quick start

```bash
# Inspect default development roots without deleting anything
devclean scan --all --global-caches --docker

# Export a standalone HTML audit
devclean scan --all --global-caches \
  --format html --output devclean-audit.html

# Conservative cleanup: Rust targets, node_modules, framework caches
devclean clean --yes

# Comprehensive generated-artifact cleanup plus Docker cache
# Docker volumes are still preserved.
devclean clean --all --global-caches --docker --yes \
  --report devclean-before.html
```

Run `devclean doctor` to inspect default roots, available tools, and active safety guarantees.

## What it cleans

| Category | Conservative | With `--all` | Classification evidence |
|---|:---:|:---:|---|
| Cargo `target` | Yes | Yes | Exact name plus Rust build markers |
| `node_modules` | Yes | Yes | Exact dependency-directory name |
| Framework cache | Yes | Yes | Known names such as `.next` and `.svelte-kit` |
| Build output | No | Yes | `build` below a recognized project manifest |
| Test/analysis cache | No | Yes | Exact mutation, lint, type-check, or test-cache name |
| Global tool cache | No | With `--global-caches` | Exact path on the built-in allowlist |
| Docker cache | No | With `--docker` | `docker system prune -af`, never volumes |

## What it never cleans

- Docker or container volumes.
- PostgreSQL and other database directories.
- Backup directories or archive files.
- Git, Mercurial, or Subversion metadata.
- Environment and secret files.
- User documents, downloads, photos, or messaging data.
- Ambiguous `dist`, `out`, or `coverage` directories without explicit evidence.

See [docs/SAFETY.md](docs/SAFETY.md) for the threat model and deletion invariants.

## Installation

### GitHub release

Download the archive for your platform from [the latest release](https://github.com/tuanle96/devclean/releases/latest), verify it against `SHA256SUMS`, and place `devclean` on your `PATH`.

### Cargo from Git

```bash
cargo install --git https://github.com/tuanle96/devclean --tag v0.1.0 --locked
```

### Build from source

```bash
git clone https://github.com/tuanle96/devclean.git
cd devclean
cargo install --path . --locked
```

The minimum supported Rust version is 1.85.

## Codex skill

The repository includes a companion skill at [`skills/dev-disk-cleaner`](skills/dev-disk-cleaner):

```bash
cp -R skills/dev-disk-cleaner ~/.codex/skills/dev-disk-cleaner
```

The skill makes agents scan first, save and open HTML evidence, request cleanup authority, preserve dirty worktrees and persistent data, and verify free space after cleanup.

## CLI overview

```text
devclean scan [OPTIONS] [ROOT]...
devclean clean [OPTIONS] [ROOT]...
devclean doctor
```

Useful options:

- `--category <CATEGORY>`: select one or more exact categories.
- `--all`: include recognized build and test outputs.
- `--global-caches`: include allowlisted downloaded caches.
- `--docker`: show Docker usage during scan or prune unused Docker data during clean.
- `--format table|json|html`: choose the scan output format.
- `--output <PATH>`: write a scan report to a file.
- `--report <PATH>`: save a pre-clean HTML report.
- `--yes`: skip the interactive `DELETE` confirmation.

Without explicit roots, `devclean` scans `~/Dev` and `~/Documents/Codex` when present, then falls back to the current directory.

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Architecture and contribution guidance are documented in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## Community and security

- Ask usage questions in [GitHub Discussions](https://github.com/tuanle96/devclean/discussions).
- Report normal bugs through the [issue tracker](https://github.com/tuanle96/devclean/issues).
- Report vulnerabilities privately according to [SECURITY.md](SECURITY.md).
- Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

Released under the [MIT License](LICENSE).
