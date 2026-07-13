import AppKit
import SwiftUI
import UniformTypeIdentifiers

public struct SettingsView: View {
    private enum SettingsTab: Hashable {
        case general
        case scan
        case intelligence
        case safety
    }

    /// Categories the scanner always considers. Shown read-only so people can see
    /// what DevCleaner scans instead of assuming an empty Settings means "nothing".
    private static let coreCategories: [CleanupCategory] = [
        .rustTarget, .nodeModules, .frameworkCache, .pythonCache, .pythonEnvironment,
    ]

    private static let repositoryURL = URL(string: "https://github.com/tuanle96/devclean")!
    private static let issuesURL = URL(string: "https://github.com/tuanle96/devclean/issues/new")!

    @ObservedObject private var model: AppModel
    @ObservedObject private var launchAtLogin: LaunchAtLoginController
    @AppStorage(PreferenceKeys.roots) private var roots = ""
    @AppStorage(PreferenceKeys.olderThan) private var olderThan = "7d"
    @AppStorage(PreferenceKeys.minimumSize) private var minimumSize = "100MiB"
    @AppStorage(PreferenceKeys.buildOutputs) private var buildOutputs = false
    @AppStorage(PreferenceKeys.testCaches) private var testCaches = false
    @AppStorage(PreferenceKeys.globalCaches) private var globalCaches = false
    @AppStorage(PreferenceKeys.expensiveCaches) private var expensiveCaches = false
    @AppStorage(PreferenceKeys.learningMode) private var learningMode = true
    @AppStorage(PreferenceKeys.safetyHoldDays) private var safetyHoldDays = 7
    @AppStorage(PreferenceKeys.aiInsightsEnabled) private var aiInsightsEnabled = true
    @AppStorage(PreferenceKeys.aiMonitoringEnabled) private var aiMonitoringEnabled = false
    @AppStorage(PreferenceKeys.aiInsightsProvider) private var aiInsightsProvider =
        AIProviderKind.appleOnDevice.rawValue
    @AppStorage(PreferenceKeys.anonymousDiagnostics) private var anonymousDiagnostics = false
    @AppStorage(PreferenceKeys.scanNotifications) private var scanNotifications = false
    @StateObject private var aiCredentials = AIProviderCredentialsController()
    @State private var selectedTab: SettingsTab = .general
    @State private var showingFolderImporter = false
    @State private var confirmPurgeAll = false

    public init(model: AppModel, launchAtLogin: LaunchAtLoginController) {
        self.model = model
        self.launchAtLogin = launchAtLogin
    }

    public var body: some View {
        TabView(selection: $selectedTab) {
            generalSettings
                .tabItem { Label("General", systemImage: "gearshape") }
                .tag(SettingsTab.general)

            scanSettings
                .tabItem { Label("Scan", systemImage: "magnifyingglass") }
                .tag(SettingsTab.scan)

            intelligenceSettings
                .tabItem { Label("Intelligence", systemImage: "sparkles") }
                .tag(SettingsTab.intelligence)

            safetySettings
                .tabItem { Label("Safety", systemImage: "lock.shield") }
                .tag(SettingsTab.safety)
        }
        .frame(width: 620)
        .onAppear {
            launchAtLogin.refresh()
            olderThan = ScanFilterOptions.normalizedOlderThan(olderThan)
            minimumSize = ScanFilterOptions.normalizedMinimumSize(minimumSize)
            SettingsWindowFocusCoordinator.activateWhenAvailable()
        }
        .confirmationDialog(
            "Permanently delete all safety holds?",
            isPresented: $confirmPurgeAll
        ) {
            Button(
                "Delete \(model.quarantineEntries.count) Holds — \(ByteFormatting.string(model.safetyHoldBytes))",
                role: .destructive
            ) {
                model.purgeAllSafetyHolds()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(
                "This releases disk space immediately and permanently deletes every safety hold. Nothing can be restored afterward."
            )
        }
    }

    // MARK: - General

    private var generalSettings: some View {
        Form {
            Section("App") {
                Toggle(
                    "Launch DevCleaner at Login",
                    isOn: Binding(
                        get: { launchAtLogin.isEnabledOrPendingApproval },
                        set: { launchAtLogin.setEnabled($0) }
                    )
                )
                launchAtLoginMessage
                if launchAtLogin.status == .requiresApproval {
                    Button("Open Login Items Settings") {
                        launchAtLogin.openSystemSettings()
                    }
                }
            }

            Section("Notifications") {
                Toggle("Notify Me After Background Scans", isOn: $scanNotifications)
                    .onChange(of: scanNotifications) { enabled in
                        if enabled { model.requestScanNotificationsAuthorization() }
                    }
                Text(
                    "DevCleaner rescans every few hours. When enabled, it notifies you once a scan finds a large amount of reclaimable space."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
            }

            Section("About") {
                LabeledContent("Version", value: appVersion)
                Link("View on GitHub", destination: Self.repositoryURL)
                Link("Report an Issue", destination: Self.issuesURL)
            }
        }
        .formStyle(.grouped)
        .accessibilityIdentifier("settings-general")
    }

    // MARK: - Scan

    private var scanSettings: some View {
        Form {
            Section("Scan locations") {
                if scanLocations.isEmpty {
                    Label("Default: ~/Dev and ~/Projects", systemImage: "folder")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(scanLocations, id: \.self) { path in
                        HStack {
                            Image(systemName: "folder")
                                .foregroundStyle(.secondary)
                                .accessibilityHidden(true)
                            Text((path as NSString).abbreviatingWithTildeInPath)
                                .lineLimit(1)
                                .truncationMode(.middle)
                                .help(path)
                            Spacer()
                            Button {
                                setScanLocations(scanLocations.filter { $0 != path })
                            } label: {
                                Image(systemName: "minus.circle.fill")
                            }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.secondary)
                            .accessibilityLabel("Remove \(path)")
                        }
                    }
                }
                HStack {
                    Button("Add Folder…") { showingFolderImporter = true }
                    Spacer()
                    Text("or drag folders here")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }

            Section("Eligibility") {
                Picker("Minimum age", selection: $olderThan) {
                    ForEach(ScanFilterOptions.olderThan, id: \.value) { option in
                        Text(option.label).tag(option.value)
                    }
                }
                Picker("Minimum size", selection: $minimumSize) {
                    ForEach(ScanFilterOptions.minimumSize, id: \.value) { option in
                        Text(option.label).tag(option.value)
                    }
                }
            }

            Section("Always scanned") {
                ForEach(Self.coreCategories) { category in
                    HStack {
                        Label(category.title, systemImage: category.systemImage)
                        Spacer()
                        Image(systemName: "checkmark")
                            .foregroundStyle(.secondary)
                            .accessibilityHidden(true)
                    }
                }
                Text("These rebuildable artifacts are always eligible for cleanup.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Also scan") {
                Toggle("Build outputs", isOn: $buildOutputs)
                Toggle("Test caches", isOn: $testCaches)
                Toggle("Package and tool caches", isOn: $globalCaches)
                Toggle("Runtime and model caches", isOn: $expensiveCaches)
                Text("Runtime and model caches can be expensive to download again.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .accessibilityIdentifier("settings-scan")
        .fileImporter(
            isPresented: $showingFolderImporter,
            allowedContentTypes: [.folder],
            allowsMultipleSelection: true
        ) { result in
            if case .success(let urls) = result { addScanLocations(urls) }
        }
        .dropDestination(for: URL.self) { urls, _ in
            // hasDirectoryPath only reflects a trailing slash, which Finder drops
            // don't reliably include — check the filesystem so real folders aren't
            // silently rejected.
            let folders = urls.filter { url in
                var isDirectory: ObjCBool = false
                return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory)
                    && isDirectory.boolValue
            }
            guard !folders.isEmpty else { return false }
            addScanLocations(folders)
            return true
        }
    }

    // MARK: - Intelligence

    private var intelligenceSettings: some View {
        Form {
            Section("Observation & Approvals") {
                Toggle("Observe artifact growth locally", isOn: $learningMode)
                Text(
                    "Stores 30 days of local size history and exact-folder approvals. Observation never authorizes or performs cleanup automatically."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
                Button("Reset Local Observation History") {
                    model.resetLearningData()
                }
            }

            Section("AI Recommendations") {
                Toggle("AI recommendations", isOn: $aiInsightsEnabled)
                Toggle("Monitor changed scan results", isOn: $aiMonitoringEnabled)
                    .disabled(!aiInsightsEnabled)
                Text(
                    selectedSettingsAIProvider == .deepSeek
                        ? "When monitoring is on, each changed scan sends compact artifact facts and project labels to DeepSeek. Full paths stay on this Mac."
                        : "When monitoring is on, changed scan results are prioritized automatically on this Mac."
                )
                .font(.caption)
                .foregroundStyle(.secondary)

                Picker("Provider", selection: $aiInsightsProvider) {
                    ForEach(AIProviderKind.allCases) { provider in
                        Text(provider.title).tag(provider.rawValue)
                    }
                }
                .pickerStyle(.segmented)

                if selectedSettingsAIProvider == .deepSeek {
                    deepSeekCredentials
                    Text(
                        "Uses DeepSeek V4 Flash through the OpenAI-compatible Chat Completions API. Recommendations are suggestions only; full paths and cleanup control never leave this Mac."
                    )
                    .font(.caption)
                    .foregroundStyle(.secondary)
                } else {
                    Text(
                        "Uses Apple's Foundation Models framework. Scanner facts stay on this Mac and recommendations work only when Apple Intelligence is available."
                    )
                    .font(.caption)
                    .foregroundStyle(.secondary)
                }

                Label(
                    settingsAIAvailability.title,
                    systemImage: settingsAIAvailability == .available
                        ? "checkmark.circle" : "info.circle"
                )
                .foregroundStyle(settingsAIAvailability == .available ? .green : .secondary)
                Text(settingsAIAvailability.message)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if let errorMessage = aiCredentials.errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
        }
        .formStyle(.grouped)
        .accessibilityIdentifier("settings-intelligence")
        .onChange(of: aiMonitoringEnabled) { enabled in
            if enabled, aiInsightsEnabled {
                model.generateAIRecommendations()
            }
        }
    }

    @ViewBuilder
    private var deepSeekCredentials: some View {
        if aiCredentials.hasDeepSeekKey {
            LabeledContent("DeepSeek API key") {
                HStack(spacing: 10) {
                    Label("Saved in Keychain", systemImage: "checkmark.shield")
                        .foregroundStyle(.green)
                        .labelStyle(.titleAndIcon)
                    Button("Remove", role: .destructive) {
                        aiCredentials.removeDeepSeekKey()
                    }
                    .buttonStyle(.borderless)
                }
            }
        } else {
            SecureField("DeepSeek API key", text: $aiCredentials.draftDeepSeekKey)
            Button("Save to Keychain") {
                aiCredentials.saveDeepSeekKey()
            }
            .disabled(
                aiCredentials.draftDeepSeekKey
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                    .isEmpty
            )
        }
    }

    // MARK: - Safety

    private var safetySettings: some View {
        Form {
            Section("Retention") {
                Stepper(
                    "Safety hold: \(safetyHoldDays == 0 ? "off" : "\(safetyHoldDays) days")",
                    value: $safetyHoldDays,
                    in: 0...30
                )
                Text(
                    "Cleaned artifacts stay restorable for this long before their disk space is reclaimed. Safety Hold is independent from observation."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
            }

            Section("Safety holds") {
                if model.quarantineEntries.isEmpty {
                    Text("No restorable holds.")
                        .foregroundStyle(.secondary)
                } else {
                    LabeledContent(
                        "Holding",
                        value:
                            "\(model.quarantineEntries.count) items · \(ByteFormatting.string(model.safetyHoldBytes))"
                    )
                    Text("Restore or delete individual holds from the DevCleaner menu bar, under Holds.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Button("Purge Expired Holds") {
                    model.purgeExpiredSafetyHolds()
                }
                .disabled(model.isBusy)
                Button("Delete All Holds Now…", role: .destructive) {
                    confirmPurgeAll = true
                }
                .disabled(model.isBusy || model.quarantineEntries.isEmpty)
            }

            Section("Diagnostics") {
                Toggle(
                    "Share anonymous errors with Sentry",
                    isOn: Binding(
                        get: { anonymousDiagnostics },
                        set: { value in
                            anonymousDiagnostics = value
                            model.setRemoteDiagnosticsConsent(value)
                        }
                    )
                )
                .disabled(!model.isRemoteMonitoringConfigured)
                Text(
                    model.isRemoteMonitoringConfigured
                        ? "Opt-in. Remote events contain error fingerprints and aggregate buckets only — never paths, usernames, or project names."
                        : "Sentry provider is built in but no DSN is configured. Local structured logs remain active."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
                Button("Open Local Logs") {
                    model.openLocalLogs()
                }
            }

            Section("Guardrails") {
                Label("Git-tracked files remain protected", systemImage: "lock.shield")
                Label("Docker volumes are never touched", systemImage: "externaldrive.badge.xmark")
                Label(
                    "Every clean re-checks each path with a fresh safety scan",
                    systemImage: "checkmark.shield"
                )
            }
        }
        .formStyle(.grouped)
        .accessibilityIdentifier("settings-safety")
    }

    // MARK: - Scan location helpers

    private var scanLocations: [String] {
        roots
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    private func setScanLocations(_ list: [String]) {
        roots = list.joined(separator: "\n")
    }

    private func addScanLocations(_ urls: [URL]) {
        var list = scanLocations
        for url in urls where !list.contains(url.path) {
            list.append(url.path)
        }
        setScanLocations(list)
    }

    // MARK: - Derived state

    private var appVersion: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String ?? "0.5.0"
        let build = info?["CFBundleVersion"] as? String ?? ""
        return build.isEmpty ? short : "\(short) (\(build))"
    }

    private var selectedSettingsAIProvider: AIProviderKind {
        AIProviderKind(rawValue: aiInsightsProvider) ?? .appleOnDevice
    }

    private var settingsAIAvailability: AIInsightsAvailability {
        switch selectedSettingsAIProvider {
        case .appleOnDevice:
            OnDeviceAIInsightsProvider().availability()
        case .deepSeek:
            OpenAICompatibleAIInsightsProvider(configuration: .deepSeek).availability()
        }
    }

    @ViewBuilder
    private var launchAtLoginMessage: some View {
        if let errorMessage = launchAtLogin.errorMessage {
            Text(errorMessage)
                .foregroundStyle(.red)
        } else {
            let message =
                switch launchAtLogin.status {
                case .enabled:
                    "DevCleaner opens automatically after you sign in. Quitting the app keeps it closed until the next login or manual launch."
                case .requiresApproval:
                    "macOS requires approval in System Settings before DevCleaner can open at login."
                case .notRegistered:
                    "DevCleaner will stay closed after your next login."
                case .notFound:
                    "Launch at Login is available from a built DevCleaner.app bundle."
                }
            Text(message)
        }
    }
}
