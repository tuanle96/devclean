import Foundation

public enum PreferenceKeys {
    public static let roots = "scan.roots"
    public static let olderThan = "scan.olderThan"
    public static let minimumSize = "scan.minimumSize"
    public static let buildOutputs = "scan.buildOutputs"
    public static let testCaches = "scan.testCaches"
    public static let globalCaches = "scan.globalCaches"
    public static let expensiveCaches = "scan.expensiveCaches"
    public static let learningMode = "learning.enabled"
    public static let safetyHoldDays = "learning.safetyHoldDays"
    public static let aiInsightsEnabled = "aiInsights.enabled"
    public static let aiInsightsProvider = "aiInsights.provider"
    public static let aiMonitoringEnabled = "aiInsights.monitoringEnabled"
    public static let anonymousDiagnostics = "diagnostics.anonymousSharing"
    public static let launchAtLogin = "app.launchAtLogin"
    public static let scanNotifications = "app.scanNotifications"
}

/// Constrained choices for the Scan settings, replacing free-text DSL fields.
/// Values map to strings the Rust CLI parses (`humantime` durations, `parse-size`
/// byte sizes); an empty value disables that filter.
public enum ScanFilterOptions {
    public static let olderThan: [(value: String, label: String)] = [
        ("", "Any age"),
        ("1d", "Older than 1 day"),
        ("3d", "Older than 3 days"),
        ("7d", "Older than 7 days"),
        ("14d", "Older than 14 days"),
        ("30d", "Older than 30 days"),
    ]

    public static let minimumSize: [(value: String, label: String)] = [
        ("", "Any size"),
        ("100MB", "At least 100 MB"),
        ("500MB", "At least 500 MB"),
        ("1GB", "At least 1 GB"),
        ("5GB", "At least 5 GB"),
    ]

    /// Snaps a stored value onto the nearest known option so a legacy "100MiB"
    /// still selects a sensible row instead of showing blank.
    public static func normalizedOlderThan(_ stored: String) -> String {
        olderThan.map(\.value).contains(stored) ? stored : "7d"
    }

    public static func normalizedMinimumSize(_ stored: String) -> String {
        if minimumSize.map(\.value).contains(stored) { return stored }
        return stored.isEmpty ? "" : "100MB"
    }
}

extension ScanSettings {
    @MainActor
    public static func load(from defaults: UserDefaults = .standard) -> ScanSettings {
        var categories: Set<CleanupCategory> = [
            .rustTarget, .nodeModules, .frameworkCache, .pythonCache, .pythonEnvironment,
        ]
        if defaults.bool(forKey: PreferenceKeys.buildOutputs) {
            categories.insert(.buildOutput)
        }
        if defaults.bool(forKey: PreferenceKeys.testCaches) {
            categories.insert(.testCache)
        }
        let includeGlobalCaches = defaults.bool(forKey: PreferenceKeys.globalCaches)
        let includeExpensiveCaches = defaults.bool(forKey: PreferenceKeys.expensiveCaches)
        let learningMode =
            defaults.object(forKey: PreferenceKeys.learningMode) == nil
            ? true
            : defaults.bool(forKey: PreferenceKeys.learningMode)
        let configuredHoldDays =
            defaults.object(forKey: PreferenceKeys.safetyHoldDays) == nil
            ? 7
            : defaults.integer(forKey: PreferenceKeys.safetyHoldDays)
        if includeGlobalCaches {
            categories.insert(.globalCache)
        }
        if includeExpensiveCaches {
            categories.insert(.expensiveGlobalCache)
        }

        let roots =
            defaults.string(forKey: PreferenceKeys.roots)?
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty } ?? []
        let olderThan = value(
            defaults.string(forKey: PreferenceKeys.olderThan),
            fallback: "7d"
        )
        let minimumSize = value(
            defaults.string(forKey: PreferenceKeys.minimumSize),
            fallback: "100MiB"
        )

        return ScanSettings(
            roots: roots,
            categories: categories,
            includeGlobalCaches: includeGlobalCaches,
            includeExpensiveCaches: includeExpensiveCaches,
            olderThan: olderThan,
            minimumSize: minimumSize,
            learningMode: learningMode,
            quarantineFor: configuredHoldDays > 0
                ? "\(configuredHoldDays)d"
                : nil
        )
    }

    private static func value(_ value: String?, fallback: String) -> String? {
        let normalized = value ?? fallback
        return normalized.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? nil
            : normalized
    }
}
