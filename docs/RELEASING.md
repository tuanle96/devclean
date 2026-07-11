# Releasing

1. Update `CHANGELOG.md` and `RELEASE_NOTES.md`.
2. Confirm the version in `Cargo.toml` and refresh `Cargo.lock`.
3. Run all local quality gates from `CONTRIBUTING.md`.
4. Commit release changes using Conventional Commits.
5. Create and push an annotated `vX.Y.Z` tag.
6. Let `.github/workflows/release.yml` build platform archives, generate checksums, package the Codex skill, and create the GitHub Release.
7. Verify every release asset and the published `SHA256SUMS` file.

Tags and published release assets must never be overwritten. Fix release problems with a new patch version.
