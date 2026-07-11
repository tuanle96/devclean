---
name: dev-disk-cleaner
description: Audit and safely clean disk space consumed by rebuildable development artifacts, including Rust target directories, node_modules, frontend/framework caches, build/test outputs, package-manager caches, and unused Docker images or build cache. Use when a development machine is low on disk, System Data is unexpectedly large, projects have accumulated generated files, or the user asks to scan, explain, reclaim, or automate developer storage cleanup.
---

# Dev Disk Cleaner

Use the bundled launcher for every audit or cleanup:

```bash
<skill-dir>/scripts/devclean doctor
```

Replace `<skill-dir>` with this skill's absolute directory. If the launcher reports that `devclean` is missing, stop before cleanup and install the CLI from its Rust source with `cargo install --path <devclean-source> --locked`.

## Workflow

1. Run a read-only scan before proposing or deleting anything.
2. Explain the largest categories and separate rebuildable artifacts from databases, backups, Docker volumes, and user data.
3. Obtain explicit cleanup authority unless the current request already grants it.
4. Save the pre-clean plan as HTML when reporting or performing cleanup, and open it in a browser.
5. Run the narrowest cleanup matching the authorization.
6. Scan again and report actual free-space change, retained data, and rebuild/redownload impact.

## Audit

Use comprehensive discovery for read-only audits:

```bash
<skill-dir>/scripts/devclean scan --all --global-caches --docker \
  --format html --output <output-dir>/devclean-audit.html \
  <root> [<root> ...]
```

Open the HTML file with the available browser or `open <output-dir>/devclean-audit.html` on macOS. Also capture the filesystem baseline with `df -h`.

Do not interpret every directory named `target` as Rust output. Trust the CLI classification, which requires Cargo build markers.

## Cleanup profiles

Conservative cleanup removes only Rust targets, JavaScript dependencies, and framework caches:

```bash
<skill-dir>/scripts/devclean clean --yes \
  --report <output-dir>/devclean-before.html \
  <root> [<root> ...]
```

Comprehensive generated-artifact cleanup additionally removes recognized build/test outputs and allowlisted global development caches. Add Docker only when the user accepts rebuilding or pulling images again:

```bash
<skill-dir>/scripts/devclean clean --all --global-caches --docker --yes \
  --report <output-dir>/devclean-before.html \
  <root> [<root> ...]
```

Use explicit `--category` values when authorization is narrower than either profile.

## Safety rules

- Never append `--volumes` to Docker cleanup.
- Never delete database directories, backups, archives, `.env` files, VCS metadata, or user documents.
- Never replace the CLI cleanup with broad `find ... -exec rm -rf` or `git clean -fdX` commands.
- Treat generic `build`, `dist`, `out`, `coverage`, and similarly named directories as ambiguous unless the CLI classifies them or the user approves an exact path.
- Preserve dirty worktrees. Generated cleanup must not alter tracked files.
- If an artifact reappears, identify the active producer with `ps` and `lsof` before retrying or stopping a process.
- Read [references/policy.md](references/policy.md) before changing category rules or handling an exceptional path.

## Verification

After cleanup, rerun the same scan and `df -h`. Verify that Docker volumes still exist when Docker was cleaned:

```bash
docker volume ls
docker system df
```

Report:

- free space before and after;
- estimated versus actual reclaimed space;
- categories removed;
- protected data explicitly retained;
- artifacts that regenerated and the responsible process;
- dependencies, images, or toolchains that will be downloaded or rebuilt next time.
