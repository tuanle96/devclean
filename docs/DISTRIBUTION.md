# Distribution

Release tags use `vX.Y.Z` and trigger quality-gated builds for Linux x86_64, macOS arm64/x86_64, and Windows x86_64.

Release assets include SHA-256 checksums, a CycloneDX SBOM, and GitHub artifact/SBOM attestations. Consumers can verify provenance with:

```bash
gh attestation verify <archive> -R tuanle96/devclean
```

The repository name and executable are `devclean`. Because that package name was already allocated on crates.io, Cargo distribution uses `devclean-cli` with a binary target named `devclean`.

Before publishing:

```bash
cargo publish --dry-run --locked
cargo publish --locked
```

Homebrew, Scoop, and WinGet manifests must reference immutable release URLs and the checksums from the final release assets. Never reuse a version or replace an uploaded artifact silently.
