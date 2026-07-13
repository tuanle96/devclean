public enum AppPhase: Equatable, Sendable {
    case idle
    case scanning
    case holding
    case cleaning
    case refreshing
    case restoring
    case purging

    public var activityDescription: String? {
        switch self {
        case .idle:
            nil
        case .scanning:
            "Scanning development artifacts…"
        case .holding:
            "Moving selection into safety hold…"
        case .cleaning:
            "Permanently deleting selected artifacts…"
        case .refreshing:
            "Refreshing candidates…"
        case .restoring:
            "Restoring safety hold…"
        case .purging:
            "Permanently deleting safety hold…"
        }
    }
}

public enum CleanupDisposition: Equatable, Sendable {
    case configuredSafetyHold
    case deleteImmediately
}
