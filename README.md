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
devclean scan --older-than 7d --min-size 500MiB

# Save a privacy-safe HTML audit
devclean scan --global-caches --docker \
    --redact-paths --format html --output devclean-audit.html

# Observe active artifacts and unknown cache-like project directories
devclean scan --learning --format json

# Watch roots read-only and notify when reclaimable artifacts exceed the threshold
devclean watch --threshold 5GiB --interval 1h

# Browse candidates in a read-only terminal UI; Enter only prints a clean command
devclean tui

# Preview the exact cleanup plan without confirmation or filesystem changes
devclean clean --dry-run

# Conservative cleanup: target, node_modules, framework caches, Python caches/environments
devclean clean --select --report devclean-before.html

# Keep selected artifacts restorable for seven days (space is released on purge)
  devclean clean --select --quarantine-for 7d
  devclean quarantine list
  devclean quarantine purge

  # Permanently delete one exact hold immediately
  devclean quarantine purge --id <ID>

  # Restore one exact hold through the clean UX shorthand
  devclean clean --undo <ID>

# Reclaim only enough to reach 100 GiB free
devclean clean --all --target-free 100GiB --yes

# Build cache only; volumes are untouched
devclean clean --docker --docker-older-than 168h --yes

# Broader Docker cleanup requires a distinct flag; still never volumes
devclean clean --docker-system --docker-older-than 168h --yes

# Install a platform-native timer after previewing it
devclean schedule install --every 7d --older-than 30d --min-size 1GiB --all --yes --dry-run

# Inspect aggregate local history (candidate paths are never stored)
devclean stats --days 30 --format html --output devclean-stats.html
```

Run `devclean doctor` to inspect roots, config search paths, tools, and active safety guarantees.

## What it cleans

| Category | Default clean | Opt-in | Evidence |
|---|:---:|---|---|
| Cargo `target` | Yes | — | Exact name plus Cargo build markers |
| `node_modules` | Yes | — | Exact dependency-directory name |
| Framework cache | Yes | — | Known names such as `.next` and `.svelte-kit` |
| Python bytecode/test cache | Yes | — | `__pycache__`, or `.tox`/`.nox` beside a direct Python project marker |
| Python virtual environment | Yes | — | `.venv`/`venv` directly beside `pyproject.toml`, requirements, or another dependency manifest |
| Build/test output | No | `--all` | Recognized manifest plus exact generated name |
| Gradle/CMake/Zig output | No | `--all` | Direct build manifest plus `build`, `.zig-cache`, `zig-cache`, or `zig-out` |
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
- `--approve-review-path PATH`: approve an exact observation only when it still matches a scanner-owned rule: SwiftPM `.build` beside `Package.swift`, `DerivedData` beside an Xcode project, `.gradle` beside a Gradle script, or `Pods` beside both `Podfile` and `Podfile.lock`.
- `--quarantine-for 7d`: retain selected artifacts in adjacent safety holds; this delays disk reclamation until purge.
- `--dry-run`: render the fully filtered clean plan and optional HTML report, then exit without confirmation, Docker invocation, quarantine, or deletion.
- `--undo ID`: restore one exact safety hold; shorthand for `quarantine restore ID`.

## Configuration

`devclean` loads the first existing file from `./.devclean.toml`, `./devclean.toml`, or the platform config directory. Pass `--config PATH` to select one explicitly. CLI values override config values. `devclean init` creates a reviewed starter file; `devclean config fetch <git-url-or-path>` clones a shared policy and validates it before replacing the local file.

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

[watch]
threshold = "5GiB"
interval = "1h"

[[rules]]
name = "turbo-cache"
category = "framework-cache"
directory_names = [".turbo"]
required_markers = ["package.json"]
reason = "rebuildable Turbo cache"
```

Custom rules remain evidence-based: names are exact, every marker must be a direct sibling, path traversal is rejected, and the rule is revalidated immediately before cleanup. See [`devclean.example.toml`](devclean.example.toml).

## Reports and automation

```bash
devclean scan --format table
devclean scan --format json --redact-paths
devclean scan --format jsonl --redact-paths
devclean scan --format html --output report.html --redact-paths
devclean stats --days 30 --format json
```

HTML and JSON can contain private absolute paths unless `--redact-paths` is used. JSONL emits safe candidates, review-only observations, Learning Mode observations, then a summary event.

Generate shell integrations without extra packages:

```bash
devclean completions zsh > _devclean
devclean completions bash > devclean.bash
devclean manpage --output devclean.1
```

Scheduled cleanup uses launchd on macOS, a systemd user timer on Linux, and Task Scheduler on Windows. Installation requires both `--yes` and an explicit profile; use `--dry-run` to inspect generated paths and commands first. Cleanup results are appended as JSONL under the platform log directory.

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
# Command-line tool
brew install tuanle96/tap/devclean

# Native macOS menu bar app
brew install --cask tuanle96/tap/devclean-menubar
```

The Cask installs and opens `DevCleaner.app` after installation. On its first launch, the app registers itself with macOS Launch at Login; this can be disabled at any time in DevCleaner Settings or System Settings > General > Login Items.

### Native macOS menu bar app

The SwiftUI app scans at launch and every six hours, displays growth history, separates safe and review-only observations, exposes approve/revoke controls for scanner-owned rules, accepts `Always select` / `Never clean` feedback, and offers both a restorable safety hold and a separately confirmed `Delete Now` path for immediate reclamation. Settings can restore or permanently delete one exact hold. The app registers with macOS Launch at Login by default, keeps structured local logs, bundles the same Rust helper used by the CLI, and never deletes files from Swift.

```bash
apps/macos/scripts/build-app.sh
open dist/DevCleaner.app
```

Local source builds are ad-hoc signed unless `CODE_SIGN_IDENTITY` is provided. Published menu bar archives are Developer ID signed, notarized by Apple, stapled, and Gatekeeper-verified. See [`apps/macos`](apps/macos).

### VS Code extension

The source extension in [`editors/vscode`](editors/vscode) shows reclaimable workspace bytes in the status bar, opens a privacy-explicit HTML report, and keeps preview separate from cleanup. Cleanup always requires a modal confirmation and invokes `devclean` directly without a shell.

Download the source-traceable VSIX from the [`vscode-v0.1.0` release](https://github.com/tuanle96/devclean/releases/tag/vscode-v0.1.0), then install it with `code --install-extension devclean-vscode-0.1.0.vsix`. Marketplace publication uses the same package once the `tuanle96` publisher credential is configured.

```bash
cd editors/vscode
npm ci
npm test
```

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
npm --prefix editors/vscode ci
npm --prefix editors/vscode test
```

See [architecture](docs/ARCHITECTURE.md), [safety model](docs/SAFETY.md), [observability](docs/OBSERVABILITY.md), [privacy](PRIVACY.md), [performance](docs/PERFORMANCE.md), [distribution](docs/DISTRIBUTION.md), and [contributing](CONTRIBUTING.md).

## Community and security

- Ask usage questions in [GitHub Discussions](https://github.com/tuanle96/devclean/discussions).
- Report bugs through [GitHub Issues](https://github.com/tuanle96/devclean/issues).
- Report vulnerabilities privately according to [SECURITY.md](SECURITY.md).
- Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

Released under the [MIT License](LICENSE).
