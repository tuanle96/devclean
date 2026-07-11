import Foundation

public enum CleanupCategory: String, Codable, CaseIterable, Identifiable, Sendable {
    case rustTarget = "rust-target"
    case nodeModules = "node-modules"
    case frameworkCache = "framework-cache"
    case buildOutput = "build-output"
    case testCache = "test-cache"
    case globalCache = "global-cache"
    case expensiveGlobalCache = "expensive-global-cache"

    public var id: String { rawValue }

    public var title: String {
        switch self {
        case .rustTarget: "Rust target"
        case .nodeModules: "Node modules"
        case .frameworkCache: "Framework cache"
        case .buildOutput: "Build output"
        case .testCache: "Test cache"
        case .globalCache: "Tool cache"
        case .expensiveGlobalCache: "Runtime / model cache"
        }
    }

    public var systemImage: String {
        switch self {
        case .rustTarget: "shippingbox"
        case .nodeModules: "cube"
        case .frameworkCache: "square.stack.3d.up"
        case .buildOutput: "hammer"
        case .testCache: "checkmark.seal"
        case .globalCache: "tray.full"
        case .expensiveGlobalCache: "externaldrive"
        }
    }
}

public struct CleanupCandidate: Codable, Hashable, Identifiable, Sendable {
    public let category: CleanupCategory
    public let path: String
    public let bytes: UInt64
    public let reason: String
    public let modifiedAtUnix: UInt64?

    public var id: String { path }

    enum CodingKeys: String, CodingKey {
        case category
        case path
        case bytes
        case reason
        case modifiedAtUnix = "modified_at_unix"
    }
}

public struct ScanReport: Codable, Equatable, Sendable {
    public let roots: [String]
    public let candidates: [CleanupCandidate]
    public let warnings: [String]
    public let totalBytes: UInt64
    public let protectGitTracked: Bool

    enum CodingKeys: String, CodingKey {
        case roots
        case candidates
        case warnings
        case totalBytes = "total_bytes"
        case protectGitTracked = "protect_git_tracked"
    }
}

public struct ScanSettings: Equatable, Sendable {
    public var roots: [String]
    public var categories: Set<CleanupCategory>
    public var includeGlobalCaches: Bool
    public var includeExpensiveCaches: Bool
    public var olderThan: String?
    public var minimumSize: String?

    public init(
        roots: [String] = [],
        categories: Set<CleanupCategory> = [.rustTarget, .nodeModules, .frameworkCache],
        includeGlobalCaches: Bool = false,
        includeExpensiveCaches: Bool = false,
        olderThan: String? = "7d",
        minimumSize: String? = "100MiB"
    ) {
        self.roots = roots
        self.categories = categories
        self.includeGlobalCaches = includeGlobalCaches
        self.includeExpensiveCaches = includeExpensiveCaches
        self.olderThan = olderThan
        self.minimumSize = minimumSize
    }
}

public enum ByteFormatting {
    public static func string(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }
}
