# Observability

Devclean uses a provider boundary so product code does not depend directly on a telemetry vendor. `MonitoringCenter` always writes local JSONL and starts the Sentry provider only when a DSN exists and the user opts in. Tests use isolated local stores or `NoOpAnalytics`.

## Event contract

| Event | Safe properties |
|---|---|
| `app_launched` | app version, OS major |
| `scan_completed` | safe/review/warning counts, duration bucket, byte bucket |
| `scan_failed` | phase |
| `cleanup_completed` | candidate count, byte bucket, safety-hold boolean |
| `cleanup_failed` | phase |
| `safety_hold_restored` | category, byte bucket |
| `safety_hold_purged` | count, byte bucket |
| `feedback_recorded` | decision |
| `learning_summary` | observed days, counts, recreated count, signed growth bucket |

Raw paths and localized error messages are allowed only in the private local log. Remote error reporting uses a normalized error domain/code fingerprint and an operation name. Adding a new remote property requires a privacy review and test evidence that it cannot contain user or project identity.

## Sentry configuration

Source builds can provide `DEVCLEAN_SENTRY_DSN`. Release builds read the optional `SENTRY_DSN` GitHub Actions secret and inject it into `DevcleanSentryDSN` in the final Info.plist before signing. The DSN is not sufficient to enable uploads; the user must also opt in.

The macOS SDK is configured without default PII. Sentry screenshot and view-hierarchy attachment APIs are unavailable for the macOS target. General analytics remain local; only sanitized failures and a periodic aggregate learning health signal are sent remotely.

## Operational checks

1. Confirm local log and learning files are mode 0600.
2. Verify a launch and scheduled scan create structured records.
3. Verify no remote request occurs without both DSN and consent.
4. Trigger a sanitized test error only in a dedicated Sentry project.
5. Review new event properties against `PRIVACY.md` before release.
