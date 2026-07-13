import Combine
import Foundation

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
    @Published public private(set) var lastScanDate: Date?
    @Published public private(set) var volumeTotalBytes: UInt64?
    /// Raw free space, comparable to total capacity, for the disk capacity bar.
    /// (`availableBytes` uses "important usage", which counts purgeable space and
    /// is not comparable to the total, so it must not drive the bar.)
    @Published public private(set) var volumeFreeBytes: UInt64?
    public let aiInsights: AIInsightsController

    private let client: DevcleanClient
    private let defaults: UserDefaults
    private let learningStore: LearningStore
    private let analytics: any AnalyticsService
    private let notifier = ScanNotifier()
    private var scanTask: Task<Void, Never>?
    private var observationTimer: Timer?
    private var backgroundMonitoringStarted = false
    private var pendingLearningStatus: String?

    public init(
        client: DevcleanClient = DevcleanClient(),
        defaults: UserDefaults = .standard,
        learningStore: LearningStore = LearningStore(),
        analytics: any AnalyticsService = MonitoringCenter.shared,
        aiInsights: AIInsightsController = AIInsightsController()
    ) {
        self.client = client
        self.defaults = defaults
        self.learningStore = learningStore
        self.analytics = analytics
        self.aiInsights = aiInsights
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

    public var safetyHoldDays: Int {
        defaults.object(forKey: PreferenceKeys.safetyHoldDays) == nil
            ? 7
            : defaults.integer(forKey: PreferenceKeys.safetyHoldDays)
    }

    public var activityDescription: String? {
        phase.activityDescription
    }

    public var menuBarSymbol: String {
        if errorMessage != nil {
            return "exclamationmark.triangle.fill"
        }
        return switch phase {
        case .scanning, .holding, .cleaning, .refreshing, .restoring, .purging:
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
        scanTask = Task {
            defer {
                phase = .idle
                scanTask = nil
            }
            do {
                let report = try await client.scan(
                    settings: settings,
                    approvedReviewPaths: learningStore.approvedReviewPaths
                )
                try Task.checkCancellation()
                self.report = report
                selectedPaths = Set(
                    report.candidates.compactMap { candidate in
                        learningStore.feedback(for: candidate.path) == .neverClean
                            ? nil
                            : candidate.path
                    })
                if settings.learningMode {
                    learningSummary = try learningStore.record(report: report)
                }
                monitorAIRecommendations(for: report)
                statusMessage = pendingLearningStatus ?? scanStatus(report)
                pendingLearningStatus = nil
                lastScanDate = Date()
                refreshAvailableSpace()
                await refreshQuarantine()
                notifier.notifyIfSignificant(
                    reclaimableBytes: report.totalBytes,
                    enabled: defaults.bool(forKey: PreferenceKeys.scanNotifications)
                )
                analytics.track(
                    .scanCompleted,
                    properties: [
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
                pendingLearningStatus = nil
                if Task.isCancelled || error is CancellationError {
                    statusMessage = "Scan cancelled."
                    return
                }
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "scan")
                analytics.track(.scanFailed, properties: ["phase": "scan"])
            }
        }
    }

    /// Cancels an in-flight scan and terminates the helper process.
    public func cancelScan() {
        scanTask?.cancel()
    }

    /// Replaces the current selection wholesale — used to restore what the user
    /// had checked after a transient action (e.g. an AI recommendation) is dismissed.
    public func setSelection(_ paths: Set<String>) {
        guard !isBusy else { return }
        selectedPaths = paths
    }

    /// Asks for notification permission when the user turns background scan alerts on.
    public func requestScanNotificationsAuthorization() {
        notifier.requestAuthorization()
    }

    public func cleanSelected(disposition: CleanupDisposition = .configuredSafetyHold) {
        guard !isBusy, !selectedPaths.isEmpty else { return }
        errorMessage = nil
        statusMessage = nil
        var settings = ScanSettings.load(from: defaults)
        if disposition == .deleteImmediately {
            settings.quarantineFor = nil
        }
        phase = settings.quarantineFor == nil ? .cleaning : .holding
        let paths = Array(selectedPaths)
        let bytesToProcess = selectedBytes
        Task {
            defer { phase = .idle }
            do {
                _ = try await client.clean(
                    paths: paths,
                    settings: settings,
                    approvedReviewPaths: learningStore.approvedReviewPaths
                )
                try learningStore.markCleaned(paths: paths)
                phase = .refreshing
                let refreshed = try await client.scan(
                    settings: settings,
                    approvedReviewPaths: learningStore.approvedReviewPaths
                )
                report = refreshed
                selectedPaths = Set(
                    refreshed.candidates.compactMap { candidate in
                        learningStore.feedback(for: candidate.path) == .neverClean
                            ? nil
                            : candidate.path
                    })
                if settings.learningMode {
                    learningSummary = try learningStore.record(report: refreshed)
                }
                monitorAIRecommendations(for: refreshed)
                await refreshQuarantine()
                if settings.quarantineFor != nil {
                    statusMessage =
                        "Moved \(ByteFormatting.string(bytesToProcess)) into a restorable safety hold. Space is released after purge."
                } else {
                    statusMessage = "Cleanup completed. Reclaimed about \(ByteFormatting.string(bytesToProcess))."
                }
                refreshAvailableSpace()
                analytics.track(
                    .cleanupCompleted,
                    properties: [
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
            statusMessage =
                decision == .neverClean
                ? "This path is protected by your local learning rule."
                : "This known-safe artifact will be selected by default."
            analytics.track(.feedbackRecorded, properties: ["decision": decision.rawValue])
        } catch {
            errorMessage = error.localizedDescription
            analytics.capture(error: error, operation: "feedback")
        }
    }

    public func approveReviewCandidate(_ candidate: ReviewCandidate) {
        guard !isBusy else { return }
        do {
            try learningStore.approve(candidate)
            learningSummary = learningStore.summary()
            pendingLearningStatus =
                "Approved "
                + (candidate.suggestedRule?.title ?? "review rule")
                + " for this project."
            analytics.track(
                .reviewRuleApproved,
                properties: [
                    "rule": candidate.suggestedRule?.rawValue ?? "unknown"
                ])
            scan()
        } catch {
            errorMessage = error.localizedDescription
            analytics.capture(error: error, operation: "approve_review_rule")
        }
    }

    public func revokeReviewApproval(path: String, rule: ReviewRule?) {
        guard !isBusy else { return }
        do {
            try learningStore.revokeApproval(for: path)
            learningSummary = learningStore.summary()
            selectedPaths.remove(path)
            pendingLearningStatus = "Revoked the learned cleanup approval for this project."
            analytics.track(
                .reviewRuleRevoked,
                properties: [
                    "rule": rule?.rawValue ?? "unknown"
                ])
            scan()
        } catch {
            errorMessage = error.localizedDescription
            analytics.capture(error: error, operation: "revoke_review_rule")
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
                analytics.track(
                    .safetyHoldRestored,
                    properties: [
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

    public func purgeSafetyHold(_ entry: QuarantineEntry) {
        guard !isBusy else { return }
        phase = .purging
        errorMessage = nil
        statusMessage = nil
        Task {
            defer { phase = .idle }
            do {
                let result = try await client.purgeQuarantine(id: entry.id)
                await refreshQuarantine()
                refreshAvailableSpace()
                statusMessage =
                    result.purged.isEmpty
                    ? "The safety hold was already missing."
                    : "Permanently deleted \(ByteFormatting.string(result.purgedBytes)) and released its disk space."
                if !result.purged.isEmpty {
                    analytics.track(
                        .safetyHoldPurged,
                        properties: [
                            "count": "1",
                            "bytes_bucket": Self.byteBucket(result.purgedBytes),
                            "trigger": "manual_exact_hold",
                        ])
                }
            } catch {
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "quarantine_purge_selected")
            }
        }
    }

    public func purgeAllSafetyHolds() {
        guard !isBusy, !quarantineEntries.isEmpty else { return }
        phase = .purging
        errorMessage = nil
        statusMessage = nil
        Task {
            defer { phase = .idle }
            do {
                let result = try await client.purgeAllQuarantine()
                await refreshQuarantine()
                refreshAvailableSpace()
                statusMessage =
                    "Permanently deleted \(result.purged.count) safety holds and released \(ByteFormatting.string(result.purgedBytes))."
                if !result.purged.isEmpty {
                    analytics.track(
                        .safetyHoldPurged,
                        properties: [
                            "count": String(result.purged.count),
                            "bytes_bucket": Self.byteBucket(result.purgedBytes),
                            "trigger": "manual_all_holds",
                        ])
                }
            } catch {
                errorMessage = error.localizedDescription
                analytics.capture(error: error, operation: "quarantine_purge_all")
            }
        }
    }

    public func setSelected(_ selected: Bool, candidate: CleanupCandidate) {
        guard !isBusy else { return }
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
        guard !isBusy else { return }
        selectedPaths = Set(
            report?.candidates.compactMap { candidate in
                learningStore.feedback(for: candidate.path) == .neverClean
                    ? nil
                    : candidate.path
            } ?? [])
    }

    public func selectNone() {
        guard !isBusy else { return }
        selectedPaths.removeAll()
    }

    public func selectOnly(_ candidate: CleanupCandidate) {
        guard !isBusy,
            report?.candidates.contains(where: { $0.path == candidate.path }) == true,
            learningStore.feedback(for: candidate.path) != .neverClean
        else { return }
        selectedPaths = [candidate.path]
    }

    public func generateAIRecommendations() {
        guard let report else {
            aiInsights.reset()
            return
        }
        aiInsights.generate(
            safeCandidates: visibleSafeCandidates(in: report),
            reviewCandidates: visibleReviewCandidates
        )
    }

    public func resetLearningData() {
        do {
            try learningStore.reset()
            learningSummary = .empty
            statusMessage = "Local observation and approval history was reset."
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
        let values = try? home.resourceValues(
            forKeys: [
                .volumeAvailableCapacityForImportantUsageKey,
                .volumeAvailableCapacityKey,
                .volumeTotalCapacityKey,
            ]
        )
        availableBytes = values?.volumeAvailableCapacityForImportantUsage.map(UInt64.init)
        volumeFreeBytes = values?.volumeAvailableCapacity.map { UInt64($0) }
        volumeTotalBytes = values?.volumeTotalCapacity.map { UInt64($0) }
    }

    private func refreshQuarantine() async {
        do {
            quarantineEntries = try await client.listQuarantine()
        } catch {
            analytics.capture(error: error, operation: "quarantine_list")
        }
    }

    private func monitorAIRecommendations(for report: ScanReport) {
        let insightsEnabled =
            defaults.object(forKey: PreferenceKeys.aiInsightsEnabled) == nil
            ? true
            : defaults.bool(forKey: PreferenceKeys.aiInsightsEnabled)
        aiInsights.monitor(
            safeCandidates: visibleSafeCandidates(in: report),
            reviewCandidates: report.reviewCandidates.filter {
                learningStore.feedback(for: $0.path) != .neverClean
            },
            enabled: insightsEnabled && defaults.bool(forKey: PreferenceKeys.aiMonitoringEnabled)
        )
    }

    private func visibleSafeCandidates(in report: ScanReport) -> [CleanupCandidate] {
        report.candidates.filter {
            learningStore.feedback(for: $0.path) != .neverClean
        }
    }

    private func purgeExpiredSafetyHolds(showStatus: Bool) async {
        do {
            let result = try await client.purgeExpiredQuarantine()
            await refreshQuarantine()
            if showStatus {
                statusMessage =
                    result.purged.isEmpty
                    ? "No expired safety holds."
                    : "Purged \(result.purged.count) holds and reclaimed \(ByteFormatting.string(result.purgedBytes))."
            }
            if !result.purged.isEmpty {
                analytics.track(
                    .safetyHoldPurged,
                    properties: [
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
            return "No eligible artifacts found. Observation mode will keep watching growth."
        }
        if report.reviewCandidates.isEmpty {
            return "Found \(report.candidates.count) rebuildable artifacts."
        }
        return
            "Found \(report.candidates.count) safe artifacts and \(report.reviewCandidates.count) review-only observations."
    }

    private func emitLearningSummary() {
        analytics.track(
            .learningSummary,
            properties: [
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
