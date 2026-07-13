import AppKit
import Foundation
import Testing

@testable import DevcleanMenuBarKit

@MainActor
@Test
func settingsFocusSelectsTitledWindowInsteadOfMenuPanel() {
    let menuPanel = NSPanel(
        contentRect: .zero,
        styleMask: [.nonactivatingPanel],
        backing: .buffered,
        defer: false
    )
    let settingsWindow = NSWindow(
        contentRect: .zero,
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
    )

    let selected = SettingsWindowFocusCoordinator.settingsWindow(
        in: [menuPanel, settingsWindow]
    )

    #expect(selected === settingsWindow)
    #expect(SettingsWindowFocusCoordinator.menuPanels(in: [menuPanel, settingsWindow]) == [menuPanel])
}

@MainActor
@Test
func cleanupConfirmationRunsActionBeforeLeavingPresentedState() {
    let confirmation = CleanupConfirmationCoordinator()
    var actionCount = 0

    confirmation.present()
    #expect(confirmation.isPresented)
    #expect(confirmation.stage == .chooseMethod)

    confirmation.requestImmediateDeletion()
    #expect(confirmation.stage == .confirmImmediateDeletion)

    confirmation.confirm {
        actionCount += 1
    }

    #expect(!confirmation.isPresented)
    #expect(actionCount == 1)

    confirmation.present()
    confirmation.cancel()
    #expect(!confirmation.isPresented)
    #expect(actionCount == 1)
}

@Test
func menuChoosesTheMostActionableInitialSection() {
    #expect(MenuSection.initial(safeCount: 2, reviewCount: 4, holdCount: 8) == .clean)
    #expect(MenuSection.initial(safeCount: 0, reviewCount: 4, holdCount: 8) == .holds)
    #expect(MenuSection.initial(safeCount: 0, reviewCount: 4, holdCount: 0) == .review)
    #expect(MenuSection.initial(safeCount: 0, reviewCount: 0, holdCount: 0) == .clean)
}

@Test
func safetyHoldExpiryRoundsUpAndReadsNaturally() {
    let now = Date(timeIntervalSince1970: 1_000_000)
    func expires(inDays days: Double) -> UInt64 {
        UInt64(now.timeIntervalSince1970 + days * 86_400)
    }
    #expect(UIFormatting.daysUntil(expires(inDays: 5), now: now) == 5)
    #expect(UIFormatting.daysUntil(expires(inDays: 0.5), now: now) == 1)
    #expect(UIFormatting.daysUntil(expires(inDays: -3), now: now) == 0)
    #expect(UIFormatting.expiryText(expires(inDays: 5), now: now) == "Expires in 5 days")
    #expect(UIFormatting.expiryText(expires(inDays: 0.5), now: now) == "Expires tomorrow")
    #expect(UIFormatting.expiryText(expires(inDays: 0), now: now) == "Expires today")
}

@Test
func capacityBarBuildsHonestDiskSegments() {
    let segments = CapacityBar.segments(
        total: 100,
        free: 50,
        held: 20,
        reclaimable: 10
    )
    let byID = Dictionary(uniqueKeysWithValues: (segments ?? []).map { ($0.id, $0.bytes) })
    #expect(byID["reclaimable"] == 10)
    #expect(byID["held"] == 20)
    #expect(byID["other"] == 20)  // total - free - held - reclaimable
    #expect(byID["free"] == 50)
    // Every slice sums back to the total capacity — an honest bar.
    let totalBytes = byID.values.reduce(0, +)
    #expect(totalBytes == 100)
    // Zero-byte slices are dropped, and missing totals disable the bar entirely.
    #expect(CapacityBar.segments(total: 100, free: 100, held: 0, reclaimable: 0)?.count == 1)
    #expect(CapacityBar.segments(total: nil, free: 50, held: 0, reclaimable: 0) == nil)
    // When free + held + reclaimable exceed the total (e.g. scan roots on another
    // volume), slices are clamped so the bar still sums to exactly the total.
    let over = CapacityBar.segments(total: 100, free: 90, held: 30, reclaimable: 20) ?? []
    #expect(over.map(\.bytes).reduce(0, +) == 100)
    #expect(over.allSatisfy { $0.bytes <= 100 })
}

@Test
func projectNamePrefersScannerRootOverParentDirectory() {
    #expect(
        UIFormatting.projectName(forPath: "/Users/me/Dev/VibeTG/target") == "VibeTG"
    )
    #expect(
        UIFormatting.projectName(
            forPath: "/Users/me/Dev/mono/apps/web/.build",
            projectRoot: "/Users/me/Dev/mono"
        ) == "mono"
    )
    #expect(
        UIFormatting.projectName(forPath: "/Users/me/Dev/mono/apps/web/.build", projectRoot: "")
            == "web"
    )
}

@Test
func menuBarStateSymbolsAllExistInSFSymbols() {
    // The badge family keeps one silhouette across states; a typo here would
    // silently render an empty menu bar icon, so validate against the system.
    let symbols = [
        "externaldrive",
        "externaldrive.badge.checkmark",
        "externaldrive.badge.minus",
        "externaldrive.badge.questionmark",
        "externaldrive.trianglebadge.exclamationmark",
    ]
    for name in symbols {
        #expect(
            NSImage(systemSymbolName: name, accessibilityDescription: nil) != nil,
            "missing SF Symbol: \(name)"
        )
    }
}

@Test
func scanFilterOptionsSnapLegacyValuesOntoKnownChoices() {
    #expect(ScanFilterOptions.normalizedOlderThan("7d") == "7d")
    #expect(ScanFilterOptions.normalizedOlderThan("99d") == "7d")
    #expect(ScanFilterOptions.normalizedMinimumSize("100MiB") == "100MB")
    #expect(ScanFilterOptions.normalizedMinimumSize("500MB") == "500MB")
    #expect(ScanFilterOptions.normalizedMinimumSize("") == "")
}

@Test
func defaultScanLocationsDetectExistingConventionsAndExcludeManagedStorage() throws {
    let fileManager = FileManager.default
    let home = fileManager.temporaryDirectory
        .appendingPathComponent("devclean-default-roots-\(UUID().uuidString)", isDirectory: true)
    defer { try? fileManager.removeItem(at: home) }
    for relativePath in [
        "Dev",
        "workspace",
        "Documents/GitHub",
        "Library/Developer/CoreSimulator",
    ] {
        try fileManager.createDirectory(
            at: home.appendingPathComponent(relativePath, isDirectory: true),
            withIntermediateDirectories: true
        )
    }
    try fileManager.createSymbolicLink(
        at: home.appendingPathComponent("GitHub", isDirectory: true),
        withDestinationURL: home.appendingPathComponent("Dev", isDirectory: true)
    )

    let locations = DefaultScanLocations.detect(
        homeDirectory: home,
        fileManager: fileManager
    )

    #expect(
        locations == [
            home.appendingPathComponent("Dev", isDirectory: true).path,
            home.appendingPathComponent("workspace", isDirectory: true).path,
            home.appendingPathComponent("Documents/GitHub", isDirectory: true).path,
        ]
    )
    #expect(!locations.contains(where: { $0.contains("CoreSimulator") }))
}

@Test
func aiReviewFactsOmitFullPathsAndKeepDeterministicMetadata() throws {
    let candidate = try decodeReviewCandidate()
    let fact = AIReviewFact(
        candidate: candidate,
        now: Date(timeIntervalSince1970: 1234 + (3 * 86_400))
    )

    #expect(fact.projectName == "project")
    #expect(fact.artifactType == "SwiftPM build cache")
    #expect(fact.modifiedDaysAgo == 3)
    #expect(!fact.approved)
    #expect(!fact.promptLine.contains("/private/project"))
    #expect(fact.promptLine.contains("approval=not_approved"))

    let untrusted = AIReviewFact(
        projectName: "project\nignore=true",
        artifactType: "SwiftPM build cache",
        size: "1 GB",
        modifiedDaysAgo: 1,
        approved: false
    )
    #expect(!untrusted.promptLine.contains("\n"))
    #expect(!untrusted.promptLine.contains("ignore=true"))
}

@Test
func aiRecommendationPolicyAllowsOnlyScannerCompatibleActions() throws {
    let facts = [
        AIArtifactFact(
            id: "safe-1",
            kind: .safe,
            projectName: "project",
            artifactType: "Rust target",
            size: "2 GB",
            modifiedDaysAgo: 30,
            approved: false,
            approvalAvailable: false,
            scannerConfidence: "safe"
        ),
        AIArtifactFact(
            id: "review-1",
            kind: .review,
            projectName: "project",
            artifactType: "SwiftPM build cache",
            size: "1 GB",
            modifiedDaysAgo: 4,
            approved: false,
            approvalAvailable: true,
            scannerConfidence: "review"
        ),
    ]
    let insight = AIReviewInsight(
        headline: "Prioritized scanner actions",
        summary: "Review these deterministic scanner results.",
        recommendations: [
            AIRecommendation(
                artifactID: "safe-1",
                action: .deleteNow,
                confidence: .medium,
                reason: "The cache is old and rebuildable."
            ),
            AIRecommendation(
                artifactID: "review-1",
                action: .deleteNow,
                confidence: .high,
                reason: "This review-only result should not be deletable."
            ),
            AIRecommendation(
                artifactID: "unknown-1",
                action: .protect,
                confidence: .high,
                reason: "Unknown targets must be ignored."
            ),
        ]
    )

    let sanitized = try AIInsightSafetyPolicy.sanitize(insight, facts: facts)

    #expect(sanitized.recommendations.count == 1)
    #expect(sanitized.recommendations[0].artifactID == "safe-1")
    #expect(sanitized.recommendations[0].action == .hold)
}

private struct DisabledAIInsightsProvider: AIInsightsProviding {
    func availability() -> AIInsightsAvailability {
        .appleIntelligenceNotEnabled
    }

    func summarizeReview(
        facts: [AIReviewFact],
        locale: Locale
    ) async throws -> AIReviewInsight {
        throw AIInsightsError.modelUnavailable(.appleIntelligenceNotEnabled)
    }
}

private actor RecordingAIRecommendationsProvider: AIInsightsProviding {
    private var requests = 0

    nonisolated func availability() -> AIInsightsAvailability {
        .available
    }

    func summarizeReview(
        facts: [AIArtifactFact],
        locale: Locale
    ) async throws -> AIReviewInsight {
        requests += 1
        let fact = try #require(facts.first)
        return AIReviewInsight(
            headline: "Recommendation ready",
            summary: "The latest scan has one prioritized action.",
            recommendations: [
                AIRecommendation(
                    artifactID: fact.id,
                    action: fact.kind == .safe ? .hold : .keepReviewing,
                    confidence: .medium,
                    reason: "The scanner evidence should remain reviewable."
                )
            ]
        )
    }

    func requestCount() -> Int {
        requests
    }
}

private final class MemoryAISecretStore: AISecretStoring, @unchecked Sendable {
    private var secrets: [String: String] = [:]
    private let lock = NSLock()

    func read(account: String) throws -> String? {
        lock.withLock { secrets[account] }
    }

    func write(_ secret: String, account: String) throws {
        lock.withLock { secrets[account] = secret }
    }

    func delete(account: String) throws {
        lock.withLock {
            _ = secrets.removeValue(forKey: account)
        }
    }
}

@MainActor
@Test
func aiInsightsControllerUsesAnExplicitUnavailableState() async throws {
    let controller = AIInsightsController(provider: DisabledAIInsightsProvider())

    controller.generate(candidates: [try decodeReviewCandidate()])
    for _ in 0..<50 {
        if controller.state == .unavailable(.appleIntelligenceNotEnabled) { break }
        try await Task.sleep(nanoseconds: 10_000_000)
    }

    #expect(controller.state == .unavailable(.appleIntelligenceNotEnabled))
}

@MainActor
@Test
func aiMonitoringDoesNotRepeatAnUnchangedScanFingerprint() async throws {
    let provider = RecordingAIRecommendationsProvider()
    let controller = AIInsightsController(provider: provider)
    let candidate = try decodeReviewCandidate()

    controller.monitor(
        safeCandidates: [],
        reviewCandidates: [candidate],
        enabled: true
    )
    for _ in 0..<50 where await provider.requestCount() == 0 {
        try await Task.sleep(nanoseconds: 10_000_000)
    }
    controller.monitor(
        safeCandidates: [],
        reviewCandidates: [candidate],
        enabled: true
    )
    try await Task.sleep(nanoseconds: 20_000_000)

    #expect(await provider.requestCount() == 1)
    #expect(controller.generatedAutomatically)
}

@Test
func deepSeekRequestUsesOpenAIJSONContractWithoutCleanupTools() throws {
    let store = MemoryAISecretStore()
    let provider = OpenAICompatibleAIInsightsProvider(
        configuration: .deepSeek,
        keyStore: store
    )
    let request = try provider.makeRequest(
        facts: [
            AIReviewFact(
                projectName: "example",
                artifactType: "SwiftPM build cache",
                size: "2 GB",
                modifiedDaysAgo: 4,
                approved: false
            )
        ],
        locale: Locale(identifier: "vi_VN"),
        apiKey: "test-secret"
    )
    let body = try #require(request.httpBody)
    let json = try #require(
        JSONSerialization.jsonObject(with: body) as? [String: Any]
    )
    let responseFormat = try #require(json["response_format"] as? [String: String])
    let thinking = try #require(json["thinking"] as? [String: String])
    let encoded = String(decoding: body, as: UTF8.self)

    #expect(request.url?.absoluteString == "https://api.deepseek.com/chat/completions")
    #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-secret")
    #expect(json["model"] as? String == "deepseek-v4-flash")
    #expect(responseFormat["type"] == "json_object")
    #expect(thinking["type"] == "disabled")
    #expect(!encoded.contains("\"tools\""))
    #expect(encoded.contains("Rust scanner remains the only cleanup authority"))
    #expect(encoded.contains("\\\"recommendations\\\""))
    #expect(encoded.contains("delete_now"))
    #expect(encoded.contains("kind=review"))
}

@Test
func configuredDeepSeekProviderRequiresAKeychainCredential() throws {
    let suiteName = "devclean-ai-provider-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    defaults.set(
        AIProviderKind.deepSeek.rawValue,
        forKey: PreferenceKeys.aiInsightsProvider
    )
    let store = MemoryAISecretStore()
    let provider = ConfiguredAIInsightsProvider(defaults: defaults, keyStore: store)

    #expect(provider.availability() == .missingAPIKey("DeepSeek"))

    try store.write("test-secret", account: AIKeychainAccount.deepSeek)
    #expect(provider.availability() == .available)
}

@MainActor
@Test
func deepSeekCredentialControllerNeverKeepsTheSavedKeyInUIState() {
    let store = MemoryAISecretStore()
    let controller = AIProviderCredentialsController(store: store)
    controller.draftDeepSeekKey = "test-secret"

    controller.saveDeepSeekKey()

    #expect(controller.hasDeepSeekKey)
    #expect(controller.draftDeepSeekKey.isEmpty)
    #expect((try? store.read(account: AIKeychainAccount.deepSeek)) == "test-secret")

    controller.removeDeepSeekKey()
    #expect(!controller.hasDeepSeekKey)
    #expect((try? store.read(account: AIKeychainAccount.deepSeek)) == nil)
}

@MainActor
@Test
func safetyHoldRetentionIsIndependentFromObservationMode() {
    let suiteName = "devclean-independent-safety-hold-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    defaults.set(false, forKey: PreferenceKeys.learningMode)
    defaults.set(7, forKey: PreferenceKeys.safetyHoldDays)

    let settings = ScanSettings.load(from: defaults)

    #expect(!settings.learningMode)
    #expect(settings.quarantineFor == "7d")
}

@Test
func appPhaseExplainsEveryBusyState() {
    #expect(AppPhase.idle.activityDescription == nil)
    #expect(AppPhase.scanning.activityDescription == "Scanning development artifacts…")
    #expect(AppPhase.holding.activityDescription == "Moving selection into safety hold…")
    #expect(
        AppPhase.cleaning.activityDescription == "Permanently deleting selected artifacts…")
    #expect(AppPhase.refreshing.activityDescription == "Refreshing candidates…")
    #expect(AppPhase.restoring.activityDescription == "Restoring safety hold…")
    #expect(AppPhase.purging.activityDescription == "Permanently deleting safety hold…")
}

@MainActor
private final class FakeLaunchAtLoginService: LaunchAtLoginServicing {
    enum Failure: Error {
        case registrationFailed
    }

    var status: LaunchAtLoginStatus
    var registerCalls = 0
    var unregisterCalls = 0
    var settingsCalls = 0
    var registrationFails = false

    init(status: LaunchAtLoginStatus) {
        self.status = status
    }

    func register() throws {
        registerCalls += 1
        if registrationFails {
            throw Failure.registrationFailed
        }
        status = .enabled
    }

    func unregister() throws {
        unregisterCalls += 1
        status = .notRegistered
    }

    func openSystemSettings() {
        settingsCalls += 1
    }
}

@MainActor
@Test
func launchAtLoginIsEnabledOnlyOnceByDefault() {
    let suiteName = "devclean-launch-at-login-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let service = FakeLaunchAtLoginService(status: .notRegistered)
    let controller = LaunchAtLoginController(service: service, defaults: defaults)

    controller.enableByDefaultIfNeeded()
    controller.enableByDefaultIfNeeded()

    #expect(service.registerCalls == 1)
    #expect(controller.status == .enabled)
    #expect(defaults.bool(forKey: PreferenceKeys.launchAtLogin))
}

@MainActor
@Test
func disabledLaunchAtLoginStaysDisabledOnNextLaunch() {
    let suiteName = "devclean-launch-at-login-disabled-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let service = FakeLaunchAtLoginService(status: .enabled)
    let first = LaunchAtLoginController(service: service, defaults: defaults)

    first.setEnabled(false)
    let reopened = LaunchAtLoginController(service: service, defaults: defaults)
    reopened.enableByDefaultIfNeeded()

    #expect(service.unregisterCalls == 1)
    #expect(service.registerCalls == 0)
    #expect(reopened.status == .notRegistered)
    #expect(!defaults.bool(forKey: PreferenceKeys.launchAtLogin))
}

@MainActor
@Test
func failedDefaultRegistrationRetriesOnNextLaunch() {
    let suiteName = "devclean-launch-at-login-retry-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suiteName)!
    defer { defaults.removePersistentDomain(forName: suiteName) }
    let service = FakeLaunchAtLoginService(status: .notRegistered)
    service.registrationFails = true
    let first = LaunchAtLoginController(service: service, defaults: defaults)

    first.enableByDefaultIfNeeded()
    #expect(defaults.object(forKey: PreferenceKeys.launchAtLogin) == nil)
    #expect(first.errorMessage != nil)

    service.registrationFails = false
    let reopened = LaunchAtLoginController(service: service, defaults: defaults)
    reopened.enableByDefaultIfNeeded()

    #expect(service.registerCalls == 2)
    #expect(reopened.status == .enabled)
    #expect(defaults.bool(forKey: PreferenceKeys.launchAtLogin))
}

@Test
func scanReportDecodesRustJSONContract() throws {
    let json = #"""
        {
          "roots": ["/Users/me/Dev"],
          "candidates": [{
            "category": "rust-target",
            "path": "/Users/me/Dev/app/target",
            "bytes": 1048576,
            "reason": "Cargo target with build markers",
            "modified_at_unix": 1234
          }],
          "warnings": [],
          "total_bytes": 1048576,
          "protect_git_tracked": true
        }
        """#

    let report = try JSONDecoder().decode(ScanReport.self, from: Data(json.utf8))

    #expect(report.candidates.first?.category == .rustTarget)
    #expect(report.totalBytes == 1_048_576)
    #expect(report.protectGitTracked)
}

@Test
func scanArgumentsUseMachineReadableOutputAndConservativeCategories() {
    let settings = ScanSettings(roots: ["~/Dev"])

    let arguments = DevcleanArguments.scan(settings: settings)

    #expect(arguments.first == "scan")
    #expect(arguments.suffix(2) == ["--format", "json"])
    #expect(arguments.contains("rust-target"))
    #expect(arguments.contains("node-modules"))
    #expect(arguments.contains("--learning"))
    #expect(!arguments.contains("--allow-tracked"))
}

@Test
func cleanArgumentsContainOnlyExactPathsAndNonInteractiveConfirmation() {
    let settings = ScanSettings(roots: ["/Users/me/Dev"])

    let arguments = DevcleanArguments.clean(
        paths: ["/Users/me/Dev/b/node_modules", "/Users/me/Dev/a/target"],
        settings: settings
    )

    #expect(arguments.first == "clean")
    #expect(arguments.filter { $0 == "--only-path" }.count == 2)
    #expect(arguments.last == "--yes")
    #expect(arguments.contains("--quarantine-for"))
    #expect(arguments.contains("7d"))
    #expect(!arguments.contains("--allow-tracked"))
    #expect(!arguments.contains("--docker-system"))
}

@Test
func immediateDeleteAndExactHoldPurgeArgumentsAreExplicit() {
    var settings = ScanSettings(roots: ["/Users/me/Dev"])
    settings.quarantineFor = nil

    let clean = DevcleanArguments.clean(
        paths: ["/Users/me/Dev/app/target"],
        settings: settings
    )
    let purge = DevcleanArguments.purgeQuarantine(id: "hold-123")

    #expect(!clean.contains("--quarantine-for"))
    #expect(clean.last == "--yes")
    #expect(purge == ["quarantine", "purge", "--id", "hold-123", "--json"])
    #expect(
        DevcleanArguments.purgeAllQuarantine
            == ["quarantine", "purge", "--all", "--json"])
}

@Test
func scanAndCleanArgumentsForwardApprovedReviewPaths() {
    let settings = ScanSettings(roots: ["/Users/me/Dev"])
    let approved = ["/Users/me/Dev/tool/.build"]

    let scan = DevcleanArguments.scan(
        settings: settings,
        approvedReviewPaths: approved
    )
    let clean = DevcleanArguments.clean(
        paths: approved,
        settings: settings,
        approvedReviewPaths: approved
    )

    #expect(scan.contains("--approve-review-path"))
    #expect(clean.contains("--approve-review-path"))
}

@Test
func locatorPrefersExplicitExecutableOverride() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-locator-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let executable = directory.appendingPathComponent("devclean")
    FileManager.default.createFile(atPath: executable.path, contents: Data())
    try FileManager.default.setAttributes(
        [.posixPermissions: 0o755],
        ofItemAtPath: executable.path
    )

    let located = DevcleanLocator.locate(
        environment: ["DEVCLEAN_EXECUTABLE": executable.path],
        homeDirectory: directory
    )

    #expect(located == executable)
}

@MainActor
@Test
func learningStoreTracksGrowthAndReviewCandidatesAcrossDays() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-learning-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = LearningStore(stateURL: directory.appendingPathComponent("learning.json"))
    let first = try decodeReport(safeBytes: 1_000_000_000, reviewBytes: nil)
    let second = try decodeReport(safeBytes: 2_000_000_000, reviewBytes: 500_000_000)

    _ = try store.record(report: first, at: Date(timeIntervalSince1970: 1_700_000_000))
    let summary = try store.record(
        report: second,
        at: Date(timeIntervalSince1970: 1_700_086_400)
    )

    #expect(summary.observedDays == 2)
    #expect(summary.reviewCount == 1)
    #expect(summary.growthBytes == 1_500_000_000)
}

@MainActor
@Test
func learningFeedbackPersistsWithoutLeavingLocalStore() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-feedback-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let stateURL = directory.appendingPathComponent("learning.json")
    let first = LearningStore(stateURL: stateURL)
    try first.recordFeedback(.neverClean, path: "/private/project/cache")

    let reopened = LearningStore(stateURL: stateURL)

    #expect(reopened.feedback(for: "/private/project/cache") == .neverClean)
}

@MainActor
@Test
func approvedReviewRulePersistsForExactScannerSuggestedPath() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-approval-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let stateURL = directory.appendingPathComponent("learning.json")
    let candidate = try decodeReviewCandidate()
    let first = LearningStore(stateURL: stateURL)

    try first.approve(candidate)
    let reopened = LearningStore(stateURL: stateURL)

    #expect(reopened.approvedRule(for: candidate.path) == .swiftPackageBuild)
    #expect(reopened.approvedReviewPaths == [candidate.path])
}

@MainActor
@Test
func versionOneLearningStateMigratesWithoutLosingFeedback() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-migration-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let stateURL = directory.appendingPathComponent("learning.json")
    let legacy = #"""
        {
          "version": 1,
          "snapshots": [],
          "feedback": {"/private/project/cache": "never-clean"},
          "cleanedAtUnix": {}
        }
        """#
    try Data(legacy.utf8).write(to: stateURL)

    let migrated = LearningStore(stateURL: stateURL)

    #expect(migrated.feedback(for: "/private/project/cache") == .neverClean)
    #expect(migrated.approvedReviewPaths.isEmpty)
}

@MainActor
@Test
func neverCleanFeedbackRevokesExistingApproval() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-never-clean-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = LearningStore(stateURL: directory.appendingPathComponent("learning.json"))
    let candidate = try decodeReviewCandidate()
    try store.approve(candidate)

    try store.recordFeedback(.neverClean, path: candidate.path)

    #expect(store.approvedRule(for: candidate.path) == nil)
}

@Test
func legacyReviewCandidateDecodesAsNotApproved() throws {
    let json = #"""
        {
          "path": "/private/project/dist",
          "bytes": 10,
          "reason": "cache-like directory",
          "modified_at_unix": 1234,
          "confidence": "review"
        }
        """#

    let candidate = try JSONDecoder().decode(ReviewCandidate.self, from: Data(json.utf8))

    #expect(!candidate.approved)
}

@MainActor
@Test
func localDiagnosticsWritesStructuredJSONLine() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-logs-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let logger = LocalDiagnosticsLogger(logDirectory: directory)

    logger.write(level: "info", event: "scan_completed", properties: ["safe_count": "2"])

    let data = try Data(contentsOf: directory.appendingPathComponent("devclean.jsonl"))
    let text = String(decoding: data, as: UTF8.self)
    #expect(text.contains("\"event\":\"scan_completed\""))
    #expect(text.contains("\"safe_count\":\"2\""))
}

private func decodeReport(safeBytes: UInt64, reviewBytes: UInt64?) throws -> ScanReport {
    let review =
        reviewBytes.map { bytes in
            #"""
            [{
              "path": "/private/project/dist",
              "bytes": \#(bytes),
              "reason": "cache-like directory",
              "modified_at_unix": 1234,
              "confidence": "review"
            }]
            """#
        } ?? "[]"
    let json = #"""
        {
          "roots": ["/private/project"],
          "candidates": [{
            "category": "rust-target",
            "path": "/private/project/target",
            "bytes": \#(safeBytes),
            "reason": "Cargo target",
            "modified_at_unix": 1234,
            "confidence": "safe"
          }],
          "review_candidates": \#(review),
          "warnings": [],
          "total_bytes": \#(safeBytes),
          "review_total_bytes": \#(reviewBytes ?? 0),
          "protect_git_tracked": true
        }
        """#
    return try JSONDecoder().decode(ScanReport.self, from: Data(json.utf8))
}

@MainActor
@Test
func learningStateFromNewerAppKeepsFeedbackWhenRuleIsUnknown() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-forward-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let stateURL = directory.appendingPathComponent("learning.json")
    let future = #"""
        {
          "version": 2,
          "snapshots": [],
          "feedback": {"/private/project/cache": "never-clean"},
          "cleanedAtUnix": {},
          "approvedRules": {
            "/private/project/.build": "swift-package-build",
            "/private/project/.bazel-out": "rule-from-a-newer-app"
          }
        }
        """#
    try Data(future.utf8).write(to: stateURL)

    let store = LearningStore(stateURL: stateURL)

    #expect(store.feedback(for: "/private/project/cache") == .neverClean)
    #expect(store.approvedRule(for: "/private/project/.build") == .swiftPackageBuild)
    #expect(store.approvedRule(for: "/private/project/.bazel-out") == nil)
}

@Test
func unknownScannerRuleDecodesAsUnsuggestedInsteadOfFailing() throws {
    let json = #"""
        {
          "path": "/private/project/.novel-cache",
          "bytes": 1000,
          "reason": "large cache-like directory beneath a recognized project",
          "confidence": "review",
          "suggested_rule": "rule-from-a-newer-helper",
          "approved": false
        }
        """#

    let candidate = try JSONDecoder().decode(ReviewCandidate.self, from: Data(json.utf8))

    #expect(candidate.suggestedRule == nil)
    #expect(!candidate.approved)
}

private func decodeReviewCandidate() throws -> ReviewCandidate {
    let json = #"""
        {
          "path": "/private/project/.build",
          "bytes": 1000,
          "reason": "large cache-like directory beneath a recognized project",
          "modified_at_unix": 1234,
          "confidence": "review",
          "suggested_rule": "swift-package-build",
          "project_root": "/private/project",
          "approved": false
        }
        """#
    return try JSONDecoder().decode(ReviewCandidate.self, from: Data(json.utf8))
}
