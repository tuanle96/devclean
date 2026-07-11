# devclean

[![CI](https://github.com/tuanle96/devclean/actions/workflows/ci.yml/badge.svg)](https://github.com/tuanle96/devclean/actions/workflows/ci.yml)
[![Security](https://github.com/tuanle96/devclean/actions/workflows/security.yml/badge.svg)](https://github.com/tuanle96/devclean/actions/workflows/security.yml)
[![Release](https://img.shields.io/github/v/release/tuanle96/devclean)](https://github.com/tuanle96/devclean/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-65d6ad.svg)](LICENSE)
[![MSRV: 1.85](https://img.shields.io/badge/MSRV-1.85-79b8ff.svg)](Cargo.toml)

`devclean` is a safety-first Rust CLI and native SwiftUI macOS menu bar app for auditing and removing rebuildable development artifacts. Learning Mode measures local growth, surfaces unknown cache-like directories as review-only observations, and can promote an exact path only through a scanner-owned rule approved by the user. Structured local diagnostics and opt-in, privacy-filtered Sentry monitoring keep failures visible.

## Safety by construction

1. Scan read-only and classify candidates using filesystem evidence.
2. Block candidates containing Git-tracked files by default.
3. Show an exact, size-sorted cleanup plan.
4. Require confirmation unless `--yes` is explicit.
5. Revalidate containment, category, type, and Git state immediately before deletion.
6. Atomically rename each candidate into a same-filesystem quarantine before recursive removal.
7. Never pass `--volumes` to Docker cleanup.
8. Keep Learning Mode observations separate from cleanup authority.

## Quick start

```bash
# Audit generated artifacts older than one week and at least 500 MiB
devclean scan --all --older-than 7d --min-size 500MiB

# Save a privacy-safe HTML audit
devclean scan --all --global-caches --docker \
    --redact-paths --format html --output devclean-audit.html

# Observe active artifacts and unknown cache-like project directories
devclean scan --learning --format json

# Conservative cleanup: target, node_modules, framework caches
devclean clean --select --report devclean-before.html

# Keep selected artifacts restorable for seven days (space is released on purge)
devclean clean --select --quarantine-for 7d
devclean quarantine list
devclean quarantine purge

# Reclaim only enough to reach 100 GiB free
devclean clean --all --target-free 100GiB --yes

# Build cache only; volumes are untouched
devclean clean --docker --docker-older-than 168h --yes

# Broader Docker cleanup requires a distinct flag; still never volumes
devclean clean --docker-system --docker-older-than 168h --yes
```

Run `devclean doctor` to inspect roots, config search paths, tools, and active safety guarantees.

## What it cleans

| Category | Default clean | Opt-in | Evidence |
|---|:---:|---|---|
| Cargo `target` | Yes | — | Exact name plus Cargo build markers |
| `node_modules` | Yes | — | Exact dependency-directory name |
| Framework cache | Yes | — | Known names such as `.next` and `.svelte-kit` |
| Build/test output | No | `--all` | Recognized manifest plus exact generated name |
| Ambiguous cache-like output | Never | `--learning` observes only | Project marker plus names such as `dist`, `out`, `.cache`, or `coverage` |
| Package/tool cache | No | `--global-caches` | Exact platform-aware allowlist |
| Model/runtime cache | No | `--expensive-caches` | Separate allowlist because redownload cost is high |
| Docker build cache | No | `--docker` | `docker builder prune`, never volumes |
| Docker system data | No | `--docker-system` | Stopped containers, unused images/networks/cache; never volumes |

Ambiguous `dist`, `out`, and `coverage` directories can appear as Learning Mode review-only observations but never enter a cleanup plan. Archives, user data, databases, VCS metadata, and Docker volumes remain protected.

## Filters and selection

- `--older-than 30d`: require the newest observed file to be old enough.
- `--min-size 1GiB`: ignore small candidates.
- `--exclude 'vendor/**'`: skip matching absolute, root-relative, or basename paths.
- `--select`: choose candidate numbers and ranges interactively.
- `--only-path PATH`: clean exact paths emitted by a previous JSON scan; every path must pass a fresh scan or the operation aborts.
- `--target-free 100GiB`: select only enough largest candidates to reach a free-space target on the first root filesystem.
- `--allow-tracked`: explicit escape hatch for vendored/generated content committed to Git.
- `--learning`: measure active known artifacts independently of age filters and surface large unknown cache-like directories as review-only.
- `--approve-review-path PATH`: approve an exact observation only when it still matches a scanner-owned rule such as SwiftPM `.build` beside `Package.swift`.
- `--quarantine-for 7d`: retain selected artifacts in adjacent safety holds; this delays disk reclamation until purge.

## Configuration

`devclean` loads the first existing file from `./devclean.toml` or the platform config directory. Pass `--config PATH` to select one explicitly. CLI values override config values.

```toml
[scan]
roots = ["/Users/me/Dev"]
exclude = ["vendor/**", "archive/**"]
older_than = "14d"
min_size = "100MiB"
max_depth = 24

[clean]
protect_git_tracked = true
expensive_caches = false
```

See [`devclean.example.toml`](devclean.example.toml).

## Reports and automation

```bash
devclean scan --format table
devclean scan --format json --redact-paths
devclean scan --format jsonl --redact-paths
devclean scan --format html --output report.html --redact-paths
```

HTML and JSON can contain private absolute paths unless `--redact-paths` is used. JSONL emits safe candidates, review-only observations, then a summary event.

Generate shell integrations without extra packages:

```bash
devclean completions zsh > _devclean
devclean completions bash > devclean.bash
devclean manpage --output devclean.1
```

## Installation

### GitHub release

Download the archive for your platform from [the latest release](https://github.com/tuanle96/devclean/releases/latest), verify `SHA256SUMS`, then verify build provenance:

```bash
gh attestation verify devclean-*.tar.gz -R tuanle96/devclean
```

### Cargo

The crates.io package is named `devclean-cli`; the installed executable is `devclean`.

```bash
cargo install devclean-cli --locked
```

### Homebrew

```bash
brew install tuanle96/tap/devclean
```

### Native macOS menu bar app

The SwiftUI app scans at launch and every six hours, displays growth history, separates safe and review-only observations, exposes approve/revoke controls for scanner-owned rules, accepts `Always select` / `Never clean` feedback, manages restorable safety holds, and keeps structured local logs. It bundles the same Rust helper used by the CLI and never deletes files from Swift.

```bash
apps/macos/scripts/build-app.sh
open dist/Devclean.app
```

Local source builds are ad-hoc signed unless `CODE_SIGN_IDENTITY` is provided. Published menu bar archives are Developer ID signed, notarized by Apple, stapled, and Gatekeeper-verified. See [`apps/macos`](apps/macos).

### Build from source

```bash
git clone https://github.com/tuanle96/devclean.git
cd devclean
cargo install --path . --locked
```

The minimum supported Rust version is 1.85.

## Codex skill

The companion skill lives at [`skills/dev-disk-cleaner`](skills/dev-disk-cleaner):

```bash
cp -R skills/dev-disk-cleaner ~/.codex/skills/dev-disk-cleaner
```

It standardizes audit, authorization, narrow cleanup, HTML evidence, regeneration diagnosis, and post-clean verification.

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo package --locked
swift test --package-path apps/macos
apps/macos/scripts/build-app.sh
```

See [architecture](docs/ARCHITECTURE.md), [safety model](docs/SAFETY.md), [observability](docs/OBSERVABILITY.md), [privacy](PRIVACY.md), [performance](docs/PERFORMANCE.md), [distribution](docs/DISTRIBUTION.md), and [contributing](CONTRIBUTING.md).

## Community and security

- Ask usage questions in [GitHub Discussions](https://github.com/tuanle96/devclean/discussions).
- Report bugs through [GitHub Issues](https://github.com/tuanle96/devclean/issues).
- Report vulnerabilities privately according to [SECURITY.md](SECURITY.md).
- Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

Released under the [MIT License](LICENSE).
