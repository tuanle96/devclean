---
name: dev-disk-cleaner
description: Audit and safely clean disk space consumed by rebuildable development artifacts, including Rust targets, node_modules, framework/build/test outputs, package caches, model/runtime caches, and unused Docker data. Use when a development machine is low on disk, System Data is unexpectedly large, generated directories have accumulated, or the user asks to scan, explain, selectively reclaim, or automate developer storage cleanup.
---

# Dev Disk Cleaner

Use the bundled launcher:

```bash
<skill-dir>/scripts/devclean doctor
```

Replace `<skill-dir>` with this skill's absolute directory. If the launcher cannot find `devclean`, stop before cleanup and install `devclean-cli` from crates.io or the repository source.

## Workflow

1. Capture `df -h` and run a read-only scan.
2. When evaluating recurring growth or missing rules, add `--learning` and treat `review_candidates` as evidence only.
3. Save and open an HTML report when reporting or cleaning.
4. Explain the largest candidates, their age, rebuild/redownload cost, confidence, and protected data.
5. Obtain cleanup authority unless the current request already grants it.
6. Apply the narrowest categories, excludes, age, size, and Docker mode matching that authority.
7. Clean or create a time-limited safety hold, scan again, verify Docker volumes, and report actual free-space change.

## Audit

```bash
<skill-dir>/scripts/devclean scan --all --global-caches --docker \
  --format html --output <output-dir>/devclean-audit.html \
  <root> [<root> ...]
```

Add `--redact-paths` before sharing a report. Use `--older-than` and `--min-size` to reduce noise, not to weaken classification.

For a multi-day evaluation, run `scan --learning --format json`. Known active artifacts may appear in `learning_observations` even when age filters keep them out of `candidates`. Unknown cache-like directories appear only in `review_candidates`. Pass `--approve-review-path` only when the report contains a `suggested_rule` and the user approves that exact path; Rust will refuse arbitrary approvals.

## Cleanup profiles

Use conservative cleanup for Rust targets, JavaScript dependencies, and framework caches:

```bash
<skill-dir>/scripts/devclean clean --select \
  --report <output-dir>/devclean-before.html \
  <root> [<root> ...]
```

Use comprehensive cleanup only when build/test output and global caches are authorized:

```bash
<skill-dir>/scripts/devclean clean --all --global-caches --yes \
  --older-than 7d --min-size 100MiB \
  --report <output-dir>/devclean-before.html \
  <root> [<root> ...]
```

Use `--target-free <SIZE>` when the user specifies a free-space goal. Use explicit `--category`, `--exclude`, or `--config` when authorization is path- or project-specific.

Use `--quarantine-for 7d` when the user prioritizes recovery over immediate reclamation. Explain that a persistent safety hold still consumes disk until `quarantine purge`. Use `quarantine list`, `restore <ID>`, and `purge` rather than manipulating `.devclean-quarantine-*` paths directly.

## Expensive and Docker data

- Add `--expensive-caches` only when the user accepts redownloading model/runtime data.
- Use `--docker` for unused build cache.
- Use `--docker-system` only when the user also accepts losing stopped containers, unused images, and unused networks.
- Add `--docker-older-than 168h` to preserve recent Docker data.
- Never append `--volumes`.

## Safety rules

- Keep Git tracked-file protection enabled. Use `--allow-tracked` only for an exact, user-approved candidate after reviewing `git status` and tracked files.
- Never delete databases, backups, archives, `.env` files, VCS metadata, Docker volumes, or user documents.
- Never replace the CLI with broad `find ... rm -rf` or `git clean -fdX` commands.
- Treat `dist`, `out`, `coverage`, and other Learning Mode review observations as ambiguous; they are not cleanable until a narrow product rule with tests is added.
- Preserve dirty worktrees and stop if a candidate contains tracked or uncommitted valuable data.
- If an artifact reappears, identify the producer with `ps` and `lsof`; do not stop IDEs, agents, or watchers without authority.
- Read [references/policy.md](references/policy.md) before changing classification or handling exceptional paths.

## Verification

```bash
df -h
<skill-dir>/scripts/devclean scan --all --global-caches <root> [<root> ...]
docker volume ls
docker system df
```

Report free space before/after, estimated and actual reclaimed bytes, categories removed, protected data retained, failures, regenerated artifacts, and future rebuild/redownload cost.
