# Architecture

`devclean` separates discovery, policy, deletion, external tools, and rendering so safety decisions remain reviewable and testable.

## Modules

- `main.rs`: CLI parsing, confirmation, command orchestration, and file output.
- `model.rs`: serializable categories, candidates, and reports.
- `scanner.rs`: bounded traversal, evidence-based classification, and allocated-size measurement.
- `cleaner.rs`: containment checks, category revalidation, and deletion.
- `docker.rs`: read-only Docker usage and volume-preserving prune commands.
- `render.rs`: terminal, JSON, and standalone HTML reports.

## Cleanup lifecycle

```text
roots
  -> bounded read-only traversal
  -> evidence-based candidates
  -> exact cleanup plan
  -> user confirmation
  -> candidate revalidation
  -> filesystem deletion / Docker prune
  -> post-clean verification
```

The scan report is not treated as permanent authority. The cleaner verifies that each directory still exists, is not a symlink, remains under an approved root, and still matches its original category.

## Size accounting

On Unix, allocated blocks are counted and hard-linked inodes are deduplicated. On other platforms, logical file length is used. Traversal does not follow symlinks and remains on the starting filesystem.

## Adding a category

1. Define a narrow evidence rule in `scanner.rs`.
2. Decide whether the category belongs in conservative defaults.
3. Add a positive detection test.
4. Add a lookalike or protected-path rejection test.
5. Update README, safety documentation, skill policy, and changelog.
