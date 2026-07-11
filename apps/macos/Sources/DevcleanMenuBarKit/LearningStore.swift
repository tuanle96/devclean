import Foundation

public enum FeedbackDecision: String, Codable, Sendable {
    case alwaysClean = "always-clean"
    case neverClean = "never-clean"
}

public struct LearningSummary: Equatable, Sendable {
    public let observedDays: Int
    public let safeCount: Int
    public let reviewCount: Int
    public let totalObservedBytes: UInt64
    public let growthBytes: Int64
    public let recreatedCount: Int
    public let feedbackCount: Int

    public static let empty = LearningSummary(
        observedDays: 0,
        safeCount: 0,
        reviewCount: 0,
        totalObservedBytes: 0,
        growthBytes: 0,
        recreatedCount: 0,
        feedbackCount: 0
    )
}

private struct LearningObservation: Codable {
    let path: String
    let bytes: UInt64
    let confidence: Confidence
}

private struct LearningSnapshot: Codable {
    let observedAtUnix: UInt64
    let observations: [LearningObservation]
}

private struct LearningState: Codable {
    var version = 1
    var snapshots: [LearningSnapshot] = []
    var feedback: [String: FeedbackDecision] = [:]
    var cleanedAtUnix: [String: UInt64] = [:]
}

@MainActor
public final class LearningStore {
    private let stateURL: URL
    private var state: LearningState
    private let calendar: Calendar

    public init(
        fileManager: FileManager = .default,
        stateURL: URL? = nil,
        calendar: Calendar = .current
    ) {
        self.calendar = calendar
        let selectedURL = stateURL ?? fileManager.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Devclean", isDirectory: true)
            .appendingPathComponent("learning.json")
        self.stateURL = selectedURL
        if let data = try? Data(contentsOf: selectedURL),
           let decoded = try? JSONDecoder().decode(LearningState.self, from: data),
           decoded.version == 1
        {
            state = decoded
        } else {
            state = LearningState()
        }
    }

    public func record(report: ScanReport, at date: Date = Date()) throws -> LearningSummary {
        let now = UInt64(max(0, date.timeIntervalSince1970))
        let fallbackObservations = report.candidates.map {
                ArtifactObservation(
                    path: $0.path,
                    category: $0.category,
                    bytes: $0.bytes,
                    reason: $0.reason,
                    modifiedAtUnix: $0.modifiedAtUnix,
                    confidence: $0.confidence ?? .safe
                )
            } + report.reviewCandidates.map {
                ArtifactObservation(
                    path: $0.path,
                    category: nil,
                    bytes: $0.bytes,
                    reason: $0.reason,
                    modifiedAtUnix: $0.modifiedAtUnix,
                    confidence: $0.confidence
                )
            }
        let source = report.learningObservations.isEmpty
            ? fallbackObservations
            : report.learningObservations
        let observations = source.map {
            LearningObservation(path: $0.path, bytes: $0.bytes, confidence: $0.confidence)
        }
        let snapshot = LearningSnapshot(observedAtUnix: now, observations: observations)
        if let last = state.snapshots.last,
           now.saturatingSubtract(last.observedAtUnix) < 15 * 60
        {
            state.snapshots[state.snapshots.count - 1] = snapshot
        } else {
            state.snapshots.append(snapshot)
        }
        let cutoff = now.saturatingSubtract(30 * 86_400)
        state.snapshots.removeAll { $0.observedAtUnix < cutoff }
        state.cleanedAtUnix = state.cleanedAtUnix.filter { $0.value >= cutoff }
        if state.snapshots.count > 256 {
            state.snapshots.removeFirst(state.snapshots.count - 256)
        }
        try persist()
        return summary()
    }

    public func summary() -> LearningSummary {
        guard let current = state.snapshots.last else { return .empty }
        let first = state.snapshots.first ?? current
        let currentBytes = current.observations.reduce(UInt64(0)) { $0.saturatingAdd($1.bytes) }
        let firstBytes = first.observations.reduce(UInt64(0)) { $0.saturatingAdd($1.bytes) }
        let growth = signedDifference(currentBytes, firstBytes)
        let days = Set(state.snapshots.map {
            calendar.startOfDay(for: Date(timeIntervalSince1970: TimeInterval($0.observedAtUnix)))
        }).count
        let recreated = current.observations.reduce(into: 0) { count, observation in
            guard let cleanedAt = state.cleanedAtUnix[observation.path],
                  current.observedAtUnix > cleanedAt
            else { return }
            count += 1
        }
        return LearningSummary(
            observedDays: days,
            safeCount: current.observations.count { $0.confidence == .safe },
            reviewCount: current.observations.count { $0.confidence == .review },
            totalObservedBytes: currentBytes,
            growthBytes: growth,
            recreatedCount: recreated,
            feedbackCount: state.feedback.count
        )
    }

    public func recordFeedback(
        _ decision: FeedbackDecision,
        path: String
    ) throws {
        state.feedback[path] = decision
        try persist()
    }

    public func feedback(for path: String) -> FeedbackDecision? {
        state.feedback[path]
    }

    public func markCleaned(paths: [String], at date: Date = Date()) throws {
        let now = UInt64(max(0, date.timeIntervalSince1970))
        for path in paths {
            state.cleanedAtUnix[path] = now
        }
        try persist()
    }

    public func reset() throws {
        state = LearningState()
        try persist()
    }

    private func persist() throws {
        try FileManager.default.createDirectory(
            at: stateURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
          let data = try encoder.encode(state)
          try data.write(to: stateURL, options: .atomic)
          try FileManager.default.setAttributes(
              [.posixPermissions: 0o600],
              ofItemAtPath: stateURL.path
          )
    }

    private func signedDifference(_ left: UInt64, _ right: UInt64) -> Int64 {
        if left >= right {
            return Int64(clamping: left - right)
        }
        return -Int64(clamping: right - left)
    }
}

private extension UInt64 {
    func saturatingAdd(_ other: UInt64) -> UInt64 {
        addingReportingOverflow(other).overflow ? .max : self + other
    }

    func saturatingSubtract(_ other: UInt64) -> UInt64 {
        self >= other ? self - other : 0
    }
}
