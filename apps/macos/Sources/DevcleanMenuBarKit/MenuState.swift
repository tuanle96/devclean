import Combine

@MainActor
enum CleanupConfirmationStage: Equatable {
    case hidden
    case chooseMethod
    case confirmImmediateDeletion
}

@MainActor
final class CleanupConfirmationCoordinator: ObservableObject {
    @Published private(set) var stage: CleanupConfirmationStage = .hidden

    var isPresented: Bool { stage != .hidden }

    func present() { stage = .chooseMethod }
    func cancel() { stage = .hidden }
    func requestImmediateDeletion() { stage = .confirmImmediateDeletion }

    func confirm(perform action: () -> Void) {
        stage = .hidden
        action()
    }
}

enum MenuSection: String, CaseIterable, Identifiable {
    case clean
    case review
    case holds

    var id: String { rawValue }

    static func initial(safeCount: Int, reviewCount: Int, holdCount: Int) -> MenuSection {
        if safeCount > 0 { return .clean }
        if holdCount > 0 { return .holds }
        if reviewCount > 0 { return .review }
        return .clean
    }
}

enum HoldPurgeRequest: Equatable {
    case one(QuarantineEntry)
    case all
}

/// Distinct VoiceOver focus targets for the modal cards. Consecutive dialogs
/// (choose method → confirm deletion) need different values so the second card
/// re-triggers focus; a shared Bool would already be `true` on the transition.
enum OverlayFocusTarget: Hashable {
    case cleanupChoose
    case cleanupConfirm
    case holdPurge
    case aiInsight
}
