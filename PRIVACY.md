# Privacy

Devclean is designed for local development workstations. Scanning, classification, Learning Mode, feedback, and cleanup run on the user's machine.

## Local data

Learning Mode stores exact artifact paths, byte estimates, confidence, timestamps, per-path `always-clean` / `never-clean` feedback, scanner-owned rule approvals, and cleaned timestamps in `~/Library/Application Support/Devclean/learning.json`. The file is mode 0600, retains at most 30 days and 256 snapshots, and can be reset from Settings.

Structured logs are written to `~/Library/Logs/Devclean/devclean.jsonl`. Local errors may contain filesystem details needed for diagnosis. The log rotates at 5 MiB and keeps one previous file. “Open local logs” reveals the directory; deleting it disables no product feature.

Persistent safety holds are tracked in the platform data directory under `devclean/quarantine.json`. The private registry contains original and quarantine paths so a hold can be restored. Purging or restoring removes the corresponding registry entry.

## Remote diagnostics

Remote diagnostics are disabled by default and unavailable unless the build contains a Sentry DSN. Enabling “Share anonymous errors with Sentry” is explicit opt-in.

Remote events are limited to:

- application version and operating-system major version;
- operation and error fingerprints;
- aggregate count, duration, growth, and byte-range buckets;
- opt-in crash, hang, and session diagnostics provided by the official Sentry Cocoa SDK.

Devclean does not intentionally send paths, usernames, project or repository names, file contents, command output, user identifiers, screenshots, or view hierarchy. The Sentry SDK is configured with `sendDefaultPii = false`. Turning sharing off closes the SDK for the current process.

The operator of a build that enables Sentry is responsible for publishing its Sentry data region and retention period before distribution. Official builds without that disclosure must leave the DSN unset.

## Control and deletion

- Disable remote diagnostics in Settings at any time.
- Reset Learning Mode history in Settings.
- Delete local logs directly from the folder opened by Settings.
- Restore or purge safety holds from Settings or `devclean quarantine`.

Privacy issues can be reported through the project's security policy or GitHub issue tracker without attaching private logs publicly.
