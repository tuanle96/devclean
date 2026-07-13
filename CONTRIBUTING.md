# Contributing to devclean

Thanks for helping make developer cleanup safer.

## Before starting

- Search existing issues and discussions.
- Open an issue before a large behavioral or policy change.
- For security vulnerabilities, follow [SECURITY.md](SECURITY.md) instead of opening a public issue.

## Development setup

Install Rust 1.85 or newer, clone the repository, then run:

```bash
cargo test --all-features --locked
```

## Required quality gates

```bash
cargo fmt --all -- --check
cargo test --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Add focused tests for every classifier or deletion-policy change. Tests should prove both sides: the intended artifact is selected and a plausible lookalike is preserved.

## Safety contract

Changes must preserve these invariants:

- Scans are read-only.
- Cleanup requires confirmation unless `--yes` is explicit.
- Symlinks are never followed or deleted as candidates.
- Candidates are revalidated immediately before deletion.
- Database, backup, VCS, and volume paths remain protected.
- Docker cleanup never uses `--volumes`.
- Ambiguous generated-looking paths remain opt-in or unclassified.

Read [docs/SAFETY.md](docs/SAFETY.md) before changing discovery or deletion behavior.

## Release tags

- Core CLI and macOS releases use exact `vMAJOR.MINOR.PATCH` tags.
- VS Code extension releases use exact `vscode-vMAJOR.MINOR.PATCH` tags, and the tag version must match `editors/vscode/package.json`.
- Do not reuse one channel's tag prefix for another channel. Release workflows validate the tag again before building or publishing artifacts.

## Commits and pull requests

- Use concise Conventional Commit messages, such as `feat(scanner): detect pnpm cache`.
- Keep unrelated changes in separate commits.
- Update documentation and `CHANGELOG.md` for user-visible behavior.
- Complete the pull-request checklist and explain safety impact.
- Do not include generated `target` directories or credentials.

By contributing, you agree that your contributions are licensed under the MIT License.
