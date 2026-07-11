import Combine
import Foundation

public enum AppPhase: Equatable, Sendable {
    case idle
    case scanning
    case cleaning
    case restoring
}

@MainActor
public final class AppModel: ObservableObject {
    @Published public private(set) var phase: AppPhase = .idle
    @Published public private(set) var report: ScanReport?
    @Published public private(set) var selectedPaths: Set<String> = []
    @Published public private(set) var availableBytes: UInt64?
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var statusMessage: String?
    @Published public private(set) var learningSummary: LearningSummary = .empty
    @Published public private(set) var quarantineEntries: [QuarantineEntry] = []

    private let client: DevcleanClient
    private let defaults: UserDefaults
    private let learningStore: LearningStore
    private let analytics: any AnalyticsService
    private var observationTimer: Timer?
    private var backgroundMonitoringStarted = false

    public init(
        client: DevcleanClient = DevcleanClient(),
        defaults: UserDefaults = .standard,
        learningStore: LearningStore = LearningStore(),
        analytics: any AnalyticsService = MonitoringCenter.shared
    ) {
        self.client = client
        self.defaults = defaults
        self.learningStore = learningStore
        self.analytics = analytics
        if let monitoring = analytics as? MonitoringCenter {
            monitoring.configure(defaults: defaults)
        }
        learningSummary = learningStore.summary()
        refreshAvailableSpace()
    }

    public var isBusy: Bool { phase != .idle }

    public var selectedBytes: UInt64 {
        report?.candidates
            .filter { selectedPaths.contains($0.path) }
            .reduce(0) { $0 + $1.bytes } ?? 0
    }

    public var safetyHoldBytes: UInt64 {
        quarantineEntries.reduce(0) { $0 + $1.bytes }
    }

    public var visibleReviewCandidates: [ReviewCandidate] {
        report?.reviewCandidates.filter {
            learningStore.feedback(for: $0.path) != .neverClean
        } ?? []
    }

    public var isRemoteMonitoringConfigured: Bool {
        (analytics as? MonitoringCenter)?.isRemoteConfigured ?? false
    }

    public var usesSafetyHold: Bool {
        ScanSettings.load(from: defaults).quarantineFor != nil
    }

    public var menuBarSymbol: String {
        if errorMessage != nil {
            return "exclamationmark.triangle.fill"
        }
        return switch phase {
        case .scanning, .cleaning, .restoring:
            "arrow.triangle.2.circlepath"
        case .idle where !(report?.reviewCandidates.isEmpty ?? true):
            "magnifyingglass.circle"
        case .idle where (report?.totalBytes ?? 0) > 0:
            "externaldrive.badge.minus"
        case .idle:
            "externaldrive.badge.checkmark"
        }
    }

    public func initialLoad() {
        refreshAvailableSpace()
        guard report == nil, !isBusy else { return }
        Task {
            await purgeExpiredSafetyHolds(showStatus: false)
            scan()
        }
    }

    public func startBackgroundMonitoring() {
        guard !backgroundMonitoringStarted else { return }
        backgroundMonitoringStarted = true
        initialLoad()
        observationTimer = Timer.scheduledTimer(
            withTimeInterval: 6 * 60 * 60,
            repeats: true
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self, !self.isBusy else { return }
                await self.purgeExpiredSafetyHolds(showStatus: false)
                self.scan()
            }
        }
    }

    public func scan() {
        guard !isBusy else { return }
        phase = .scanning
        errorMessage = nil
        statusMessage = nil
        let settings = ScanSettings.load(from: defaults)
        let started = Date()
        Task {
            defer { phase = .idle }
            do {
                let report = try await client.scan(settings: settings)
                self.report = report
                selectedPaths = Set(report.candidates.compactMap { candidate in
                    learningStore.feedback(for: candidate.path) == .neverClean
                        ? nil
                        : candidate.path
                })
                if settings.learningMode {
                    learningSummary = try learningStore.record(report: report)
                }
                statusMessage = scanStatus(report)
                refreshAvailableSpace()
                await refreshQuarantine()
                analytics.track(.scanCompleted, properties: [
                    "safe_count": String(report.candidates.count),
                    "review_count": String(report.reviewCandidates.count),
                    "warning_count": String(report.warnings.count),
                    "bytes_bucket": Self.byteBucket(
                        report.observedTotalBytes > 0
                            ? report.observedTotalBytes
                            : report.totalBytes + report.reviewTotalBytes
                    ),
                    "duration_bucket": Self.durationBucket(Date().timeIntervalSince(started)),
                ])
                emitLearningSummary()
            } catch {
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "scan")
                analytics.track(.scanFailed, properties: ["phase": "scan"])
            }
        }
    }

    public func cleanSelected() {
        guard !isBusy, !selectedPaths.isEmpty else { return }
        phase = .cleaning
        errorMessage = nil
        statusMessage = nil
        let settings = ScanSettings.load(from: defaults)
        let paths = Array(selectedPaths)
        let bytesToProcess = selectedBytes
        Task {
            defer { phase = .idle }
            do {
                _ = try await client.clean(paths: paths, settings: settings)
                try learningStore.markCleaned(paths: paths)
                let refreshed = try await client.scan(settings: settings)
                report = refreshed
                selectedPaths = Set(refreshed.candidates.compactMap { candidate in
                    learningStore.feedback(for: candidate.path) == .neverClean
                        ? nil
                        : candidate.path
                })
                if settings.learningMode {
                    learningSummary = try learningStore.record(report: refreshed)
                }
                await refreshQuarantine()
                if settings.quarantineFor != nil {
                    statusMessage = "Moved \(ByteFormatting.string(bytesToProcess)) into a restorable safety hold. Space is released after purge."
                } else {
                    statusMessage = "Cleanup completed. Reclaimed about \(ByteFormatting.string(bytesToProcess))."
                }
                refreshAvailableSpace()
                analytics.track(.cleanupCompleted, properties: [
                    "candidate_count": String(paths.count),
                    "bytes_bucket": Self.byteBucket(bytesToProcess),
                    "safety_hold": settings.quarantineFor == nil ? "false" : "true",
                ])
            } catch {
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "cleanup")
                analytics.track(.cleanupFailed, properties: ["phase": "cleanup"])
            }
        }
    }

    public func recordFeedback(_ decision: FeedbackDecision, path: String) {
        do {
            try learningStore.recordFeedback(decision, path: path)
            if decision == .neverClean {
                selectedPaths.remove(path)
            } else if report?.candidates.contains(where: { $0.path == path }) == true {
                selectedPaths.insert(path)
            }
            learningSummary = learningStore.summary()
            statusMessage = decision == .neverClean
                ? "This path is protected by your local learning rule."
                : "This known-safe artifact will be selected by default."
            analytics.track(.feedbackRecorded, properties: ["decision": decision.rawValue])
        } catch {
            errorMessage = error.localizedDescription
            analytics.capture(error: error, operation: "feedback")
        }
    }

    public func feedback(for path: String) -> FeedbackDecision? {
        learningStore.feedback(for: path)
    }

    public func restoreSafetyHold(_ entry: QuarantineEntry) {
        guard !isBusy else { return }
        phase = .restoring
        errorMessage = nil
        Task {
            defer { phase = .idle }
            do {
                try await client.restoreQuarantine(id: entry.id)
                statusMessage = "Restored \(entry.originalPath)."
                await refreshQuarantine()
                analytics.track(.safetyHoldRestored, properties: [
                    "category": entry.category.rawValue,
                    "bytes_bucket": Self.byteBucket(entry.bytes),
                ])
            } catch {
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "quarantine_restore")
            }
        }
    }

    public func purgeExpiredSafetyHolds() {
        Task { await purgeExpiredSafetyHolds(showStatus: true) }
    }

    public func setSelected(_ selected: Bool, candidate: CleanupCandidate) {
        if selected {
            selectedPaths.insert(candidate.path)
        } else {
            selectedPaths.remove(candidate.path)
        }
    }

    public func isSelected(_ candidate: CleanupCandidate) -> Bool {
        selectedPaths.contains(candidate.path)
    }

    public func selectAll() {
        selectedPaths = Set(report?.candidates.compactMap { candidate in
            learningStore.feedback(for: candidate.path) == .neverClean
                ? nil
                : candidate.path
        } ?? [])
    }

    public func selectNone() {
        selectedPaths.removeAll()
    }

    public func resetLearningData() {
        do {
            try learningStore.reset()
            learningSummary = .empty
            statusMessage = "Local Learning Mode history was reset."
        } catch {
            errorMessage = error.localizedDescription
            analytics.capture(error: error, operation: "learning_reset")
        }
    }

    public func setRemoteDiagnosticsConsent(_ enabled: Bool) {
        defaults.set(enabled, forKey: PreferenceKeys.anonymousDiagnostics)
        (analytics as? MonitoringCenter)?.setRemoteConsent(enabled)
    }

    public func openLocalLogs() {
        (analytics as? MonitoringCenter)?.openLocalLogs()
    }

    public func refreshAvailableSpace() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        availableBytes = try? home.resourceValues(
            forKeys: [.volumeAvailableCapacityForImportantUsageKey]
        ).volumeAvailableCapacityForImportantUsage.map(UInt64.init)
    }

    private func refreshQuarantine() async {
        do {
            quarantineEntries = try await client.listQuarantine()
        } catch {
            analytics.capture(error: error, operation: "quarantine_list")
        }
    }

    private func purgeExpiredSafetyHolds(showStatus: Bool) async {
        do {
            let result = try await client.purgeExpiredQuarantine()
            await refreshQuarantine()
            if showStatus {
                statusMessage = result.purged.isEmpty
                    ? "No expired safety holds."
                    : "Purged \(result.purged.count) holds and reclaimed \(ByteFormatting.string(result.purgedBytes))."
            }
            if !result.purged.isEmpty {
                analytics.track(.safetyHoldPurged, properties: [
                    "count": String(result.purged.count),
                    "bytes_bucket": Self.byteBucket(result.purgedBytes),
                ])
            }
        } catch {
            if showStatus {
                errorMessage = error.localizedDescription
            }
            analytics.capture(error: error, operation: "quarantine_purge")
        }
    }

    private func scanStatus(_ report: ScanReport) -> String {
        if report.candidates.isEmpty, report.reviewCandidates.isEmpty {
            return "No eligible artifacts found. Learning Mode will keep watching growth."
        }
        if report.reviewCandidates.isEmpty {
            return "Found \(report.candidates.count) rebuildable artifacts."
        }
        return "Found \(report.candidates.count) safe artifacts and \(report.reviewCandidates.count) review-only observations."
    }

    private func emitLearningSummary() {
        analytics.track(.learningSummary, properties: [
            "observed_days": String(learningSummary.observedDays),
            "safe_count": String(learningSummary.safeCount),
            "review_count": String(learningSummary.reviewCount),
            "recreated_count": String(learningSummary.recreatedCount),
            "growth_bucket": Self.signedByteBucket(learningSummary.growthBytes),
        ])
    }

    private static func byteBucket(_ bytes: UInt64) -> String {
        switch bytes {
        case 0..<100_000_000: "under_100mb"
        case 100_000_000..<1_000_000_000: "100mb_1gb"
        case 1_000_000_000..<10_000_000_000: "1gb_10gb"
        case 10_000_000_000..<50_000_000_000: "10gb_50gb"
        default: "over_50gb"
        }
    }

    private static func signedByteBucket(_ bytes: Int64) -> String {
        let direction = bytes < 0 ? "down" : "up"
        return "\(direction)_\(byteBucket(bytes.magnitude))"
    }

    private static func durationBucket(_ seconds: TimeInterval) -> String {
        switch seconds {
        case ..<1: "under_1s"
        case ..<5: "1s_5s"
        case ..<30: "5s_30s"
        default: "over_30s"
        }
    }
}
