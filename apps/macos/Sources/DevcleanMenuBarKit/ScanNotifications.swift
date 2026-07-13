import Foundation
@preconcurrency import UserNotifications

/// Opt-in local notifications for background scans. The 6-hour background scan is
/// otherwise invisible; when the user enables notifications DevCleaner tells them
/// once a scan finds a meaningful amount of reclaimable space.
@MainActor
public final class ScanNotifier {
    private let center: UNUserNotificationCenter?
    private var lastNotifiedBytes: UInt64 = 0

    /// ~5 GB — below this a notification is more nagging than useful.
    private static let threshold: UInt64 = 5_000_000_000

    public init() {
        // UNUserNotificationCenter.current() traps when there is no application
        // bundle (e.g. unit tests / SwiftPM run). Guard on a bundle identifier.
        center = Bundle.main.bundleIdentifier == nil ? nil : .current()
    }

    /// Requests authorization the first time the user turns notifications on.
    public func requestAuthorization() {
        center?.requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    /// Posts at most one notification per newly-discovered batch of reclaimable space.
    public func notifyIfSignificant(reclaimableBytes: UInt64, enabled: Bool) {
        guard enabled, let center else { return }
        guard reclaimableBytes >= Self.threshold, reclaimableBytes != lastNotifiedBytes else {
            return
        }
        lastNotifiedBytes = reclaimableBytes
        center.getNotificationSettings { settings in
            guard settings.authorizationStatus == .authorized else { return }
            let content = UNMutableNotificationContent()
            content.title = "DevCleaner"
            content.body = "\(ByteFormatting.string(reclaimableBytes)) of build artifacts are ready to clean."
            let request = UNNotificationRequest(
                identifier: "devcleaner.scan.reclaimable",
                content: content,
                trigger: nil
            )
            center.add(request)
        }
    }
}
