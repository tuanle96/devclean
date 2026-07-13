import AppKit
import SwiftUI

public struct MenuContentView: View {
    @ObservedObject private var model: AppModel
    @StateObject private var cleanupConfirmation = CleanupConfirmationCoordinator()
    @ObservedObject private var aiInsights: AIInsightsController
    @AppStorage(PreferenceKeys.aiInsightsEnabled) private var aiInsightsEnabled = true
    @AppStorage(PreferenceKeys.aiMonitoringEnabled) private var aiMonitoringEnabled = false
    @Environment(\.colorScheme) private var colorScheme
    @State private var selectedSection: MenuSection = .clean
    @State private var holdPurgeRequest: HoldPurgeRequest?
    @State private var showingAIInsight = false
    @State private var didChooseInitialSection = false
    /// Selection captured before an AI recommendation narrows it, so cancelling
    /// the confirmation restores exactly what the user had checked.
    @State private var savedSelection: Set<String>?

    public init(model: AppModel) {
        self.model = model
        aiInsights = model.aiInsights
    }

    public var body: some View {
        ZStack {
            VStack(alignment: .leading, spacing: 12) {
                header
                sectionPicker
                aiRecommendationStatus
                sectionSummary
                content
                messages
                Divider()
                actions
                footer
            }
            .padding(16)
            .disabled(isPresentingOverlay)

            if cleanupConfirmation.isPresented {
                cleanupConfirmationOverlay
            } else if let holdPurgeRequest {
                holdPurgeConfirmationOverlay(holdPurgeRequest)
            } else if showingAIInsight {
                aiInsightOverlay
            }
        }
        .frame(width: 420)
        .onExitCommand {
            if cleanupConfirmation.isPresented {
                cancelCleanup()
            } else if showingAIInsight {
                showingAIInsight = false
            } else {
                holdPurgeRequest = nil
            }
        }
        .onAppear { chooseInitialSectionIfNeeded() }
        .onChange(of: model.isBusy) { isBusy in
            if !isBusy {
                chooseInitialSectionIfNeeded()
            }
        }
        .task { model.initialLoad() }
    }

    private var isPresentingOverlay: Bool {
        cleanupConfirmation.isPresented || holdPurgeRequest != nil || showingAIInsight
    }

    /// A dimming layer behind modal cards. Adapts to appearance (a hard black
    /// scrim is too heavy in Light Mode) and treats a tap as cancel, which is the
    /// standard expectation for dismissing a modal by clicking outside it.
    private func dimScrim(onCancel: @escaping () -> Void) -> some View {
        Color.black
            .opacity(colorScheme == .dark ? 0.45 : 0.22)
            .contentShape(Rectangle())
            .onTapGesture(perform: onCancel)
            .accessibilityHidden(true)
    }

    private func chooseInitialSectionIfNeeded() {
        guard !didChooseInitialSection, !model.isBusy, model.report != nil else { return }
        selectedSection = MenuSection.initial(
            safeCount: model.report?.candidates.count ?? 0,
            reviewCount: model.visibleReviewCandidates.count,
            holdCount: model.quarantineEntries.count
        )
        didChooseInitialSection = true
    }

    private var cleanupConfirmationOverlay: some View {
        ZStack {
            dimScrim { cancelCleanup() }

            VStack(alignment: .leading, spacing: 14) {
                cleanupConfirmationContent
            }
            .padding(20)
            .frame(width: 310)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
            .overlay {
                RoundedRectangle(cornerRadius: 18)
                    .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
            .padding(20)
            .accessibilityAddTraits(.isModal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .zIndex(1)
        .accessibilityElement(children: .contain)
    }

    private func holdPurgeConfirmationOverlay(_ request: HoldPurgeRequest) -> some View {
        ZStack {
            dimScrim { holdPurgeRequest = nil }

            VStack(alignment: .leading, spacing: 14) {
                Text(holdPurgeTitle(for: request))
                    .font(.headline)

                Text(holdPurgeMessage(for: request))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                Button(role: .destructive) {
                    confirmHoldPurge(request)
                } label: {
                    Text(holdPurgeButtonTitle(for: request))
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(.red)
                .disabled(model.isBusy)
                .accessibilityIdentifier("hold-purge-confirm")

                Button(role: .cancel) {
                    holdPurgeRequest = nil
                } label: {
                    Text("Cancel")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .keyboardShortcut(.cancelAction)
                .accessibilityIdentifier("hold-purge-cancel")
            }
            .padding(20)
            .frame(width: 330)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
            .overlay {
                RoundedRectangle(cornerRadius: 18)
                    .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
            .padding(20)
            .accessibilityAddTraits(.isModal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .zIndex(1)
        .accessibilityElement(children: .contain)
    }

    private func holdPurgeTitle(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one:
            return "Permanently delete this hold?"
        case .all:
            return "Delete all safety holds?"
        }
    }

    private func holdPurgeMessage(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one(let entry):
            return
                "This releases \(ByteFormatting.string(entry.bytes)) immediately and cannot be undone. The original path will no longer be restorable."
        case .all:
            return
                "This permanently deletes all \(model.quarantineEntries.count) restorable items and releases \(ByteFormatting.string(model.safetyHoldBytes)). This cannot be undone."
        }
    }

    private func holdPurgeButtonTitle(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one(let entry):
            return "Delete \(ByteFormatting.string(entry.bytes)) Now"
        case .all:
            return "Delete All · \(ByteFormatting.string(model.safetyHoldBytes))"
        }
    }

    private func confirmHoldPurge(_ request: HoldPurgeRequest) {
        holdPurgeRequest = nil
        switch request {
        case .one(let entry):
            model.purgeSafetyHold(entry)
        case .all:
            model.purgeAllSafetyHolds()
        }
    }

    private var aiInsightOverlay: some View {
        ZStack {
            dimScrim { showingAIInsight = false }

            VStack(alignment: .leading, spacing: 14) {
                aiInsightContent

                Divider()

                Label(
                    aiInsightPrivacyFooter,
                    systemImage: "lock.shield"
                )
                .font(.caption2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

                Button(role: .cancel) {
                    showingAIInsight = false
                } label: {
                    Text("Close")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .keyboardShortcut(.cancelAction)
                .accessibilityIdentifier("ai-insight-close")
            }
            .padding(20)
            .frame(width: 390)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
            .overlay {
                RoundedRectangle(cornerRadius: 18)
                    .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
            .padding(20)
            .accessibilityAddTraits(.isModal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .zIndex(1)
        .accessibilityElement(children: .contain)
    }

    @ViewBuilder
    private var aiInsightContent: some View {
        switch aiInsights.state {
        case .idle:
            Text("Recommend next actions")
                .font(.headline)
            Text(aiInsightIntro)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            aiInsightGenerateButton(title: "Generate Recommendations")
        case .generating:
            HStack(spacing: 10) {
                ProgressView()
                    .controlSize(.small)
                VStack(alignment: .leading, spacing: 2) {
                    Text(
                        selectedAIProviderKind == .appleOnDevice
                            ? "Prioritizing on device…" : "Requesting recommendations…"
                    )
                    .font(.headline)
                    Text(aiInsightProcessingMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, minHeight: 72, alignment: .leading)
        case .result(let insight):
            Label(insight.headline, systemImage: "sparkles")
                .font(.headline)
                .foregroundStyle(.purple)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
            Text(insight.summary)
                .font(.subheadline)
                .fixedSize(horizontal: false, vertical: true)
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(insight.recommendations) { recommendation in
                        recommendationRow(recommendation)
                    }
                }
            }
            .frame(maxHeight: 330)
            aiInsightGenerateButton(title: "Refresh Recommendations")
        case .unavailable(let availability):
            Label(availability.title, systemImage: "apple.intelligence")
                .font(.headline)
            Text(availability.message)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            aiInsightGenerateButton(title: "Check Again")
        case .failed(let message):
            Label("AI recommendations could not be generated", systemImage: "exclamationmark.triangle")
                .font(.headline)
                .foregroundStyle(.orange)
            Text(message)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            aiInsightGenerateButton(title: "Try Again")
        }
    }

    private func recommendationRow(_ recommendation: AIRecommendation) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack(spacing: 7) {
                Label(recommendation.action.title, systemImage: recommendation.action.systemImage)
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text(recommendation.confidence.rawValue.uppercased())
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(recommendation.confidence == .high ? .green : .secondary)
            }
            Text(recommendationTargetTitle(recommendation))
                .font(.caption.weight(.medium))
                .lineLimit(1)
            Text(recommendation.reason)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            if recommendation.action != .keepReviewing {
                Button(recommendationButtonTitle(recommendation.action)) {
                    applyAIRecommendation(recommendation)
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .tint(recommendation.action == .deleteNow ? .red : .accentColor)
                .disabled(model.isBusy)
                .accessibilityIdentifier("ai-action-\(recommendation.id)")
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 10))
    }

    private func recommendationTargetTitle(_ recommendation: AIRecommendation) -> String {
        guard let target = aiInsights.target(for: recommendation) else {
            return "Scanner result changed — refresh recommendations"
        }
        switch target {
        case .safe(let candidate):
            let project = URL(fileURLWithPath: candidate.path).deletingLastPathComponent().lastPathComponent
            return "\(candidate.category.title) · \(project) · \(ByteFormatting.string(candidate.bytes))"
        case .review(let candidate):
            let projectPath =
                candidate.projectRoot
                ?? URL(fileURLWithPath: candidate.path).deletingLastPathComponent().path
            let project = URL(fileURLWithPath: projectPath).lastPathComponent
            return
                "\(candidate.suggestedRule?.title ?? "Review artifact") · \(project) · \(ByteFormatting.string(candidate.bytes))"
        }
    }

    private func recommendationButtonTitle(_ action: AIRecommendationAction) -> String {
        switch action {
        case .hold:
            "Review Hold…"
        case .deleteNow:
            "Review Delete…"
        case .approve:
            "Approve Exact Rule"
        case .protect:
            "Protect This Path"
        case .keepReviewing:
            "Keep Reviewing"
        }
    }

    private func applyAIRecommendation(_ recommendation: AIRecommendation) {
        guard let target = aiInsights.target(for: recommendation) else {
            aiInsights.reset()
            return
        }
        switch (recommendation.action, target) {
        case (.hold, .safe(let candidate)):
            savedSelection = model.selectedPaths
            model.selectOnly(candidate)
            showingAIInsight = false
            cleanupConfirmation.present()
        case (.deleteNow, .safe(let candidate)):
            savedSelection = model.selectedPaths
            model.selectOnly(candidate)
            showingAIInsight = false
            cleanupConfirmation.present()
            cleanupConfirmation.requestImmediateDeletion()
        case (.approve, .review(let candidate)):
            showingAIInsight = false
            model.approveReviewCandidate(candidate)
        case (.protect, .safe(let candidate)):
            showingAIInsight = false
            model.recordFeedback(.neverClean, path: candidate.path)
            aiInsights.reset()
        case (.protect, .review(let candidate)):
            showingAIInsight = false
            model.recordFeedback(.neverClean, path: candidate.path)
            aiInsights.reset()
        case (.keepReviewing, .review):
            showingAIInsight = false
        default:
            aiInsights.reset()
        }
    }

    private func aiInsightGenerateButton(title: String) -> some View {
        Button {
            model.generateAIRecommendations()
        } label: {
            Text(title)
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .disabled(
            (model.report?.candidates.isEmpty ?? true)
                && model.visibleReviewCandidates.isEmpty
        )
        .accessibilityIdentifier("ai-recommend-generate")
    }

    private func presentAIInsight() {
        showingAIInsight = true
        if case .result = aiInsights.state {
            return
        }
        model.generateAIRecommendations()
    }

    private var selectedAIProviderKind: AIProviderKind {
        AIProviderKind.selected()
    }

    private var aiInsightIntro: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "Apple Intelligence ranks the current results on this Mac. Nothing is cleaned until you choose a recommendation."
        case .deepSeek:
            "DeepSeek ranks compact facts about the current results. Full paths stay on this Mac and every action still needs your click."
        }
    }

    private var aiInsightProcessingMessage: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "Nothing is sent to a server."
        case .deepSeek:
            "Compact facts and project labels are sent to DeepSeek; full paths stay on this Mac."
        }
    }

    private var aiInsightPrivacyFooter: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "On-device suggestions · every action needs your click · re-checked before deletion"
        case .deepSeek:
            "Remote suggestions · no full paths leave this Mac · every action needs your click"
        }
    }

    @ViewBuilder
    private var cleanupConfirmationContent: some View {
        switch cleanupConfirmation.stage {
        case .hidden:
            EmptyView()
        case .chooseMethod:
            Text("Clean selected artifacts?")
                .font(.headline)

            Text(
                model.usesSafetyHold
                    ? "Keep a restorable \(model.safetyHoldDays)-day safety hold, or permanently delete now to reclaim the space immediately. DevCleaner re-verifies every path either way."
                    : "Safety holds are disabled. Continue to a final confirmation before permanent deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            if model.usesSafetyHold {
                Button {
                    savedSelection = nil
                    cleanupConfirmation.confirm {
                        model.cleanSelected(disposition: .configuredSafetyHold)
                    }
                } label: {
                    Text(
                        "Hold \(ByteFormatting.string(model.selectedBytes)) for \(model.safetyHoldDays) \(model.safetyHoldDays == 1 ? "Day" : "Days")"
                    )
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(model.isBusy || model.selectedPaths.isEmpty)
                .accessibilityIdentifier("cleanup-hold")
            }

            Button(role: .destructive) {
                cleanupConfirmation.requestImmediateDeletion()
            } label: {
                Text("Delete \(ByteFormatting.string(model.selectedBytes)) Now…")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
            .accessibilityIdentifier("cleanup-delete-now")

            cleanupCancelButton
        case .confirmImmediateDeletion:
            Text("Permanently delete selected artifacts?")
                .font(.headline)

            Text(
                "This reclaims the space immediately and cannot be undone or restored. DevCleaner re-verifies every selected path right before deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            Button(role: .destructive) {
                savedSelection = nil
                cleanupConfirmation.confirm {
                    model.cleanSelected(disposition: .deleteImmediately)
                }
            } label: {
                Text("Permanently Delete \(ByteFormatting.string(model.selectedBytes))")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
            .accessibilityIdentifier("cleanup-delete-confirm")

            Button {
                cleanupConfirmation.returnToMethods()
            } label: {
                Text("Back")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .accessibilityIdentifier("cleanup-delete-back")

            cleanupCancelButton
        }
    }

    private var cleanupCancelButton: some View {
        Button(role: .cancel) {
            cancelCleanup()
        } label: {
            Text("Cancel")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
        .keyboardShortcut(.cancelAction)
        .accessibilityIdentifier("cleanup-cancel")
    }

    /// Dismisses the cleanup confirmation and restores any selection an AI
    /// recommendation temporarily narrowed.
    private func cancelCleanup() {
        cleanupConfirmation.cancel()
        if let saved = savedSelection {
            model.setSelection(saved)
            savedSelection = nil
        }
    }

    private var sectionPicker: some View {
        Picker("Task", selection: $selectedSection) {
            Text(sectionLabel("Clean", count: model.report?.candidates.count ?? 0))
                .tag(MenuSection.clean)
            Text(sectionLabel("Review", count: model.visibleReviewCandidates.count))
                .tag(MenuSection.review)
            Text(sectionLabel("Holds", count: model.quarantineEntries.count))
                .tag(MenuSection.holds)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .accessibilityIdentifier("task-picker")
    }

    private func sectionLabel(_ title: String, count: Int) -> String {
        count > 0 ? "\(title) \(count)" : title
    }

    @ViewBuilder
    private var aiRecommendationStatus: some View {
        if aiInsightsEnabled, aiMonitoringEnabled {
            switch aiInsights.state {
            case .generating:
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text("AI is reviewing the latest scan…")
                        .font(.caption)
                    Spacer()
                }
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("ai-monitoring-progress")
            case .result(let insight):
                Button {
                    showingAIInsight = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "sparkles")
                            .foregroundStyle(.purple)
                        Text("\(insight.recommendations.count) AI recommendations ready")
                            .font(.caption.weight(.semibold))
                        Spacer()
                        Text("View")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(9)
                    .background(.purple.opacity(0.1), in: RoundedRectangle(cornerRadius: 9))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("ai-monitoring-result")
            case .idle, .unavailable, .failed:
                EmptyView()
            }
        }
    }

    @ViewBuilder
    private var sectionSummary: some View {
        switch selectedSection {
        case .clean:
            if let report = model.report, !report.candidates.isEmpty {
                HStack {
                    Label(
                        "\(report.candidates.count) safe artifacts",
                        systemImage: "checkmark.shield"
                    )
                    Spacer()
                    if aiInsightsEnabled {
                        Button {
                            presentAIInsight()
                        } label: {
                            Label("Recommend", systemImage: "sparkles")
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(model.isBusy)
                        .accessibilityIdentifier("ai-recommend-open-clean")
                    }
                    Button("All") { model.selectAll() }
                        .buttonStyle(.plain)
                        .disabled(model.isBusy)
                    Text("·").foregroundStyle(.tertiary)
                    Button("None") { model.selectNone() }
                        .buttonStyle(.plain)
                        .disabled(model.isBusy)
                }
                .font(.subheadline)
            }
        case .review:
            HStack(spacing: 6) {
                Image(systemName: "checkmark.bubble")
                    .foregroundStyle(.orange)
                    .accessibilityHidden(true)
                Text("Observation & Approvals")
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text(observedForText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if aiInsightsEnabled, !model.visibleReviewCandidates.isEmpty {
                    Button {
                        presentAIInsight()
                    } label: {
                        Label("Recommend", systemImage: "sparkles")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(model.isBusy)
                    .accessibilityIdentifier("ai-recommend-open-review")
                }
            }
        case .holds:
            HStack(spacing: 10) {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.title3)
                    .foregroundStyle(.teal)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(ByteFormatting.string(model.safetyHoldBytes)) held safely")
                        .font(.subheadline.weight(.semibold))
                    Text("Restore items, or permanently delete them to reclaim space now.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
        }
    }

    private var observedForText: String {
        let days = model.learningSummary.observedDays
        if days <= 0 { return "Not yet observed" }
        return "Observing for \(days) \(days == 1 ? "day" : "days")"
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 9) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("DevCleaner")
                        .font(.headline)
                    Text(subtitleText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(headlineValue)
                        .font(.title3.weight(.semibold).monospacedDigit())
                        .foregroundStyle(headlineColor)
                        .contentTransition(.numericText())
                    Text(headlineLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
            }
            capacityBar
        }
    }

    @ViewBuilder
    private var capacityBar: some View {
        if let total = model.volumeTotalBytes, total > 0,
            let segments = CapacityBar.segments(
                total: total,
                free: model.volumeFreeBytes,
                held: model.safetyHoldBytes,
                reclaimable: model.report?.totalBytes ?? 0
            )
        {
            VStack(alignment: .leading, spacing: 5) {
                GeometryReader { geometry in
                    HStack(spacing: 1) {
                        ForEach(segments) { segment in
                            segment.color
                                .frame(
                                    width: max(
                                        2,
                                        geometry.size.width * CGFloat(Double(segment.bytes) / Double(total))
                                    )
                                )
                        }
                    }
                }
                .frame(height: 6)
                .clipShape(Capsule())
                .accessibilityHidden(true)

                HStack(spacing: 12) {
                    ForEach(segments.filter { $0.id != "other" }) { segment in
                        HStack(spacing: 4) {
                            Circle()
                                .fill(segment.color)
                                .frame(width: 7, height: 7)
                            Text("\(ByteFormatting.string(segment.bytes)) \(segment.label)")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    private var subtitleText: String {
        if model.phase == .scanning {
            return "Scanning…"
        }
        if let date = model.lastScanDate {
            return "Last scan \(UIFormatting.relativePast(date))"
        }
        // Free space is shown by the capacity bar below; keep the subtitle about
        // scan freshness so two different "free" figures never appear together.
        return "Not scanned yet"
    }

    private var headlineValue: String {
        let reclaimable = model.report?.totalBytes ?? 0
        if reclaimable > 0 {
            return ByteFormatting.string(reclaimable)
        }
        if model.safetyHoldBytes > 0 {
            return "All clean"
        }
        return "Up to date"
    }

    private var headlineLabel: String {
        if (model.report?.totalBytes ?? 0) > 0 {
            return "ready to clean"
        }
        if model.safetyHoldBytes > 0 {
            return "\(ByteFormatting.string(model.safetyHoldBytes)) held safely"
        }
        return "nothing to reclaim"
    }

    private var headlineColor: Color {
        (model.report?.totalBytes ?? 0) > 0 ? .accentColor : .green
    }

    @ViewBuilder
    private var content: some View {
        if model.phase == .scanning, model.report == nil {
            HStack(spacing: 10) {
                ProgressView()
                Text("Scanning development artifacts…")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 120)
        } else {
            switch selectedSection {
            case .clean:
                cleanContent
            case .review:
                reviewContent
            case .holds:
                holdsContent
            }
        }
    }

    @ViewBuilder
    private var cleanContent: some View {
        if let candidates = model.report?.candidates, !candidates.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(candidates) { candidate in
                        CandidateRow(candidate: candidate, model: model)
                        if candidate.id != candidates.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: candidates.count, rowHeight: 48))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("No safe artifacts to clean")
                    .font(.headline)
                Text("Scan again, adjust filters in Settings, or manage space already in safety holds.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 300)
                HStack(spacing: 8) {
                    Button("Open Settings…") { openSettingsWindow() }
                        .buttonStyle(.bordered)
                    if !model.quarantineEntries.isEmpty {
                        Button("View \(ByteFormatting.string(model.safetyHoldBytes)) in Holds") {
                            selectedSection = .holds
                        }
                        .buttonStyle(.bordered)
                        .accessibilityIdentifier("view-holds")
                    }
                }
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    @ViewBuilder
    private var reviewContent: some View {
        if !model.visibleReviewCandidates.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(model.visibleReviewCandidates) { candidate in
                        ReviewCandidateRow(candidate: candidate, model: model)
                        if candidate.id != model.visibleReviewCandidates.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: model.visibleReviewCandidates.count, rowHeight: 52))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "checkmark.seal")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("Nothing needs review")
                    .font(.headline)
                Text("Items DevCleaner isn't sure about wait here. Approve one to allow cleaning it in future scans.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 310)
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    @ViewBuilder
    private var holdsContent: some View {
        if !model.quarantineEntries.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(model.quarantineEntries) { entry in
                        safetyHoldRow(entry)
                        if entry.id != model.quarantineEntries.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: model.quarantineEntries.count, rowHeight: 58))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "archivebox")
                    .font(.system(size: 28))
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text("No safety holds")
                    .font(.headline)
                Text(
                    "Artifacts cleaned with Safety Hold will remain restorable here until their retention period expires."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 310)
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    private func safetyHoldRow(_ entry: QuarantineEntry) -> some View {
        SafetyHoldRow(
            entry: entry,
            isBusy: model.isBusy,
            onRestore: { model.restoreSafetyHold(entry) },
            onDelete: { holdPurgeRequest = .one(entry) },
            onReveal: { FinderActions.revealInFinder(entry.originalPath) },
            onCopyPath: { FinderActions.copyPath(entry.originalPath) }
        )
    }

    @ViewBuilder
    private var messages: some View {
        if let error = model.errorMessage {
            Label(error, systemImage: "exclamationmark.triangle.fill")
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        } else if let status = model.statusMessage {
            Text(status)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        if let warnings = model.report?.warnings, !warnings.isEmpty {
            Label("\(warnings.count) paths were skipped safely", systemImage: "info.circle")
                .font(.caption)
                .foregroundStyle(.secondary)
                .help(warnings.joined(separator: "\n"))
        }
    }

    private var actions: some View {
        HStack(spacing: 8) {
            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
                Text(model.activityDescription ?? "Working…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("activity-progress")
                Spacer()
                if model.phase == .scanning {
                    Button("Cancel") { model.cancelScan() }
                        .controlSize(.small)
                        .keyboardShortcut(.cancelAction)
                        .accessibilityIdentifier("cancel-scan")
                }
            } else {
                Button {
                    model.scan()
                } label: {
                    Label("Scan", systemImage: "arrow.clockwise")
                }
                .keyboardShortcut("r")
                .help("Scan for artifacts again (⌘R)")

                Spacer()

                switch selectedSection {
                case .clean:
                    Button("Clean Selected") {
                        cleanupConfirmation.present()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(model.selectedPaths.isEmpty)
                    .accessibilityIdentifier("clean-selected")
                case .review:
                    Text("Approvals apply only to the folders you approve")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                case .holds:
                    if !model.quarantineEntries.isEmpty {
                        Button(role: .destructive) {
                            holdPurgeRequest = .all
                        } label: {
                            Label("Delete All…", systemImage: "trash")
                        }
                        .buttonStyle(.bordered)
                        .accessibilityIdentifier("hold-delete-all")
                    }
                }
            }
        }
        .controlSize(.large)
    }

    private var footer: some View {
        HStack {
            if #available(macOS 14.0, *) {
                SettingsLink {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
                .simultaneousGesture(
                    TapGesture().onEnded {
                        SettingsWindowFocusCoordinator.activateAfterSettingsLink()
                    }
                )
            } else {
                Button {
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                    SettingsWindowFocusCoordinator.activateWhenAvailable()
                } label: {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .keyboardShortcut("q")
                .help("Quit DevCleaner (⌘Q)")
        }
        .font(.caption)
    }

    private func openSettingsWindow() {
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
        SettingsWindowFocusCoordinator.activateWhenAvailable()
    }

    private func listHeight(for rowCount: Int, rowHeight: CGFloat) -> CGFloat {
        min(340, max(100, CGFloat(rowCount) * rowHeight))
    }
}
