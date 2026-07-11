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

public enum Confidence: String, Codable, Sendable {
    case safe
    case review
    case protected
}

public struct CleanupCandidate: Codable, Hashable, Identifiable, Sendable {
    public let category: CleanupCategory
    public let path: String
    public let bytes: UInt64
    public let reason: String
    public let modifiedAtUnix: UInt64?
    public let confidence: Confidence?

    public var id: String { path }

    enum CodingKeys: String, CodingKey {
        case category
        case path
        case bytes
        case reason
        case modifiedAtUnix = "modified_at_unix"
        case confidence
    }
}

public struct ReviewCandidate: Codable, Hashable, Identifiable, Sendable {
    public let path: String
    public let bytes: UInt64
    public let reason: String
    public let modifiedAtUnix: UInt64?
    public let confidence: Confidence

    public var id: String { path }

    enum CodingKeys: String, CodingKey {
        case path
        case bytes
        case reason
        case modifiedAtUnix = "modified_at_unix"
        case confidence
    }
}

public struct ArtifactObservation: Codable, Hashable, Identifiable, Sendable {
    public let path: String
    public let category: CleanupCategory?
    public let bytes: UInt64
    public let reason: String
    public let modifiedAtUnix: UInt64?
    public let confidence: Confidence

    public var id: String { path }

    enum CodingKeys: String, CodingKey {
        case path
        case category
        case bytes
        case reason
        case modifiedAtUnix = "modified_at_unix"
        case confidence
    }
}

public struct ScanReport: Codable, Equatable, Sendable {
    public let roots: [String]
    public let candidates: [CleanupCandidate]
    public let reviewCandidates: [ReviewCandidate]
    public let learningObservations: [ArtifactObservation]
    public let warnings: [String]
    public let totalBytes: UInt64
    public let reviewTotalBytes: UInt64
    public let observedTotalBytes: UInt64
    public let protectGitTracked: Bool

    enum CodingKeys: String, CodingKey {
        case roots
        case candidates
        case reviewCandidates = "review_candidates"
        case learningObservations = "learning_observations"
        case warnings
        case totalBytes = "total_bytes"
        case reviewTotalBytes = "review_total_bytes"
        case observedTotalBytes = "observed_total_bytes"
        case protectGitTracked = "protect_git_tracked"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        roots = try values.decode([String].self, forKey: .roots)
        candidates = try values.decode([CleanupCandidate].self, forKey: .candidates)
        reviewCandidates = try values.decodeIfPresent(
            [ReviewCandidate].self,
            forKey: .reviewCandidates
        ) ?? []
        learningObservations = try values.decodeIfPresent(
            [ArtifactObservation].self,
            forKey: .learningObservations
        ) ?? []
        warnings = try values.decode([String].self, forKey: .warnings)
        totalBytes = try values.decode(UInt64.self, forKey: .totalBytes)
        reviewTotalBytes = try values.decodeIfPresent(
            UInt64.self,
            forKey: .reviewTotalBytes
        ) ?? 0
        observedTotalBytes = try values.decodeIfPresent(
            UInt64.self,
            forKey: .observedTotalBytes
        ) ?? 0
        protectGitTracked = try values.decode(Bool.self, forKey: .protectGitTracked)
    }

    public func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(roots, forKey: .roots)
        try values.encode(candidates, forKey: .candidates)
        try values.encode(reviewCandidates, forKey: .reviewCandidates)
        try values.encode(learningObservations, forKey: .learningObservations)
        try values.encode(warnings, forKey: .warnings)
        try values.encode(totalBytes, forKey: .totalBytes)
        try values.encode(reviewTotalBytes, forKey: .reviewTotalBytes)
        try values.encode(observedTotalBytes, forKey: .observedTotalBytes)
        try values.encode(protectGitTracked, forKey: .protectGitTracked)
    }
}

public struct QuarantineEntry: Codable, Equatable, Identifiable, Sendable {
    public let id: String
    public let originalPath: String
    public let quarantinePath: String
    public let category: CleanupCategory
    public let bytes: UInt64
    public let createdAtUnix: UInt64
    public let expiresAtUnix: UInt64

    enum CodingKeys: String, CodingKey {
        case id
        case originalPath = "original_path"
        case quarantinePath = "quarantine_path"
        case category
        case bytes
        case createdAtUnix = "created_at_unix"
        case expiresAtUnix = "expires_at_unix"
    }
}

public struct QuarantinePurgeReport: Codable, Equatable, Sendable {
    public let purged: [QuarantineEntry]
    public let failures: [String]
    public let purgedBytes: UInt64

    enum CodingKeys: String, CodingKey {
        case purged
        case failures
        case purgedBytes = "purged_bytes"
    }
}

public struct ScanSettings: Equatable, Sendable {
    public var roots: [String]
    public var categories: Set<CleanupCategory>
    public var includeGlobalCaches: Bool
    public var includeExpensiveCaches: Bool
    public var olderThan: String?
    public var minimumSize: String?
    public var learningMode: Bool
    public var quarantineFor: String?

    public init(
        roots: [String] = [],
        categories: Set<CleanupCategory> = [.rustTarget, .nodeModules, .frameworkCache],
        includeGlobalCaches: Bool = false,
        includeExpensiveCaches: Bool = false,
        olderThan: String? = "7d",
        minimumSize: String? = "100MiB",
        learningMode: Bool = true,
        quarantineFor: String? = "7d"
    ) {
        self.roots = roots
        self.categories = categories
        self.includeGlobalCaches = includeGlobalCaches
        self.includeExpensiveCaches = includeExpensiveCaches
        self.olderThan = olderThan
        self.minimumSize = minimumSize
        self.learningMode = learningMode
        self.quarantineFor = quarantineFor
    }
}

public enum ByteFormatting {
    public static func string(_ bytes: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(clamping: bytes), countStyle: .file)
    }
}
