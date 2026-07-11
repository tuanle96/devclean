import Foundation

public enum PreferenceKeys {
    public static let roots = "scan.roots"
    public static let olderThan = "scan.olderThan"
    public static let minimumSize = "scan.minimumSize"
    public static let buildOutputs = "scan.buildOutputs"
    public static let testCaches = "scan.testCaches"
    public static let globalCaches = "scan.globalCaches"
    public static let expensiveCaches = "scan.expensiveCaches"
}

public extension ScanSettings {
    @MainActor
    static func load(from defaults: UserDefaults = .standard) -> ScanSettings {
        var categories: Set<CleanupCategory> = [.rustTarget, .nodeModules, .frameworkCache]
        if defaults.bool(forKey: PreferenceKeys.buildOutputs) {
            categories.insert(.buildOutput)
        }
        if defaults.bool(forKey: PreferenceKeys.testCaches) {
            categories.insert(.testCache)
        }
        let includeGlobalCaches = defaults.bool(forKey: PreferenceKeys.globalCaches)
        let includeExpensiveCaches = defaults.bool(forKey: PreferenceKeys.expensiveCaches)
        if includeGlobalCaches {
            categories.insert(.globalCache)
        }
        if includeExpensiveCaches {
            categories.insert(.expensiveGlobalCache)
        }

        let roots = defaults.string(forKey: PreferenceKeys.roots)?
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
            minimumSize: minimumSize
        )
    }

    private static func value(_ value: String?, fallback: String) -> String? {
        let normalized = value ?? fallback
        return normalized.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            ? nil
            : normalized
    }
}
