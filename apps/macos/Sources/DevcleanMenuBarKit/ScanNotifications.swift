import AppKit
import Foundation
@preconcurrency import UserNotifications

/// Brings the app forward when a scan notification is tapped. MenuBarExtra has no
/// public API to open its own popover, so activation plus the state-badged menu
/// bar icon is the closest supported hand-off.
private final class ScanNotificationTapHandler: NSObject, UNUserNotificationCenterDelegate {
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner])
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        DispatchQueue.main.async {
            NSApp.activate(ignoringOtherApps: true)
        }
        completionHandler()
    }
}

/// Opt-in local notifications for background scans. The 6-hour background scan is
/// otherwise invisible; when the user enables notifications DevCleaner tells them
/// once a scan finds a meaningful amount of reclaimable space.
@MainActor
public final class ScanNotifier {
    private let center: UNUserNotificationCenter?
    private let tapHandler = ScanNotificationTapHandler()
    private var lastNotifiedBytes: UInt64 = 0

    /// ~5 GB — below this a notification is more nagging than useful.
    private static let threshold: UInt64 = 5_000_000_000

    public init() {
        // UNUserNotificationCenter.current() traps when there is no application
        // bundle (e.g. unit tests / SwiftPM run). Guard on a bundle identifier.
        center = Bundle.main.bundleIdentifier == nil ? nil : .current()
        center?.delegate = tapHandler
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
