import AppKit
import SwiftUI

extension MenuContentView {
    var aiInsightOverlay: some View {
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
    var aiInsightContent: some View {
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

    func recommendationRow(_ recommendation: AIRecommendation) -> some View {
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

    func recommendationTargetTitle(_ recommendation: AIRecommendation) -> String {
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

    func recommendationButtonTitle(_ action: AIRecommendationAction) -> String {
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

    func applyAIRecommendation(_ recommendation: AIRecommendation) {
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

    func aiInsightGenerateButton(title: String) -> some View {
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

    func presentAIInsight() {
        showingAIInsight = true
        if case .result = aiInsights.state {
            return
        }
        model.generateAIRecommendations()
    }

    var selectedAIProviderKind: AIProviderKind {
        AIProviderKind.selected()
    }

    var aiInsightIntro: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "Apple Intelligence ranks the current results on this Mac. Nothing is cleaned until you choose a recommendation."
        case .deepSeek:
            "DeepSeek ranks compact facts about the current results. Full paths stay on this Mac and every action still needs your click."
        }
    }

    var aiInsightProcessingMessage: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "Nothing is sent to a server."
        case .deepSeek:
            "Compact facts and project labels are sent to DeepSeek; full paths stay on this Mac."
        }
    }

    var aiInsightPrivacyFooter: String {
        switch selectedAIProviderKind {
        case .appleOnDevice:
            "On-device suggestions · every action needs your click · re-checked before deletion"
        case .deepSeek:
            "Remote suggestions · no full paths leave this Mac · every action needs your click"
        }
    }
}
