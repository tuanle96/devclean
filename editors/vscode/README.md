# DevCleaner for VS Code

The extension exposes one left-aligned workspace status item and four commands:

- scan the current local workspace;
- open a standalone HTML report;
- preview cleanup with `clean --dry-run`;
- clean only after a modal confirmation that follows the visible dry-run.

It launches `devclean` directly with an argument array and never invokes a shell. Configure `devclean.executable` when the binary is not on `PATH`.
