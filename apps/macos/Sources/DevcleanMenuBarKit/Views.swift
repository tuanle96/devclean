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

    var isPresentingOverlay: Bool {
        cleanupConfirmation.isPresented || holdPurgeRequest != nil || showingAIInsight
    }
}
