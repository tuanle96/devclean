import AppKit
import SwiftUI

public struct MenuContentView: View {
    @ObservedObject var model: AppModel
    @StateObject var cleanupConfirmation = CleanupConfirmationCoordinator()
    @ObservedObject var aiInsights: AIInsightsController
    @AppStorage(PreferenceKeys.aiInsightsEnabled) var aiInsightsEnabled = true
    @AppStorage(PreferenceKeys.aiMonitoringEnabled) var aiMonitoringEnabled = false
    @Environment(\.colorScheme) var colorScheme
    @State var selectedSection: MenuSection = .clean
    @State var holdPurgeRequest: HoldPurgeRequest?
    @State var showingAIInsight = false
    @State var didChooseInitialSection = false
    /// Selection captured before an AI recommendation narrows it, so cancelling
    /// the confirmation restores exactly what the user had checked.
    @State var savedSelection: Set<String>?
    /// Keyboard focus for the candidate lists (arrow-key navigation).
    @State var listFocus: String?
    /// Latest read-only memory sample; refreshed only while the menu is open.
    @State var memorySnapshot: MemorySnapshot?
    /// Moves VoiceOver into a modal card the moment it is presented.
    @AccessibilityFocusState var overlayFocus: OverlayFocusTarget?

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
        // Modal cards can be taller than a short tab (e.g. an empty state), so give
        // the window room while one is up instead of clipping the card.
        .frame(minHeight: isPresentingOverlay ? 400 : nil)
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
        // A manual tab choice is final: without this, the first scan finishing
        // would yank the user off Memory (the one tab that is live mid-scan).
        .onChange(of: selectedSection) { _ in
            didChooseInitialSection = true
        }
        .task { model.initialLoad() }
        .task { await sampleMemoryWhileVisible() }
    }

    var isPresentingOverlay: Bool {
        cleanupConfirmation.isPresented || holdPurgeRequest != nil || showingAIInsight
    }
}
