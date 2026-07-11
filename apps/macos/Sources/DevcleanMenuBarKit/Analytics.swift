import AppKit
import Foundation
import Sentry

public enum AnalyticsEventName: String, Sendable {
    case appLaunched = "app_launched"
    case scanCompleted = "scan_completed"
    case scanFailed = "scan_failed"
    case cleanupCompleted = "cleanup_completed"
    case cleanupFailed = "cleanup_failed"
    case safetyHoldRestored = "safety_hold_restored"
    case safetyHoldPurged = "safety_hold_purged"
    case feedbackRecorded = "feedback_recorded"
    case reviewRuleApproved = "review_rule_approved"
    case reviewRuleRevoked = "review_rule_revoked"
    case learningSummary = "learning_summary"
}

@MainActor
public protocol AnalyticsService: AnyObject {
    func track(_ event: AnalyticsEventName, properties: [String: String])
    func capture(error: Error, operation: String)
}

@MainActor
public final class NoOpAnalytics: AnalyticsService {
    public init() {}
    public func track(_: AnalyticsEventName, properties _: [String: String]) {}
    public func capture(error _: Error, operation _: String) {}
}

private struct LocalLogRecord: Codable {
    let timestamp: String
    let level: String
    let event: String
    let properties: [String: String]
}

@MainActor
public final class LocalDiagnosticsLogger {
    public static let shared = LocalDiagnosticsLogger()

    public let logDirectory: URL
    private let logURL: URL
    private let encoder = JSONEncoder()
    private let maximumBytes: UInt64 = 5 * 1_024 * 1_024

    public init(
        fileManager: FileManager = .default,
        logDirectory: URL? = nil
    ) {
        let selectedDirectory = logDirectory ?? fileManager.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Logs/Devclean", isDirectory: true)
        self.logDirectory = selectedDirectory
        logURL = selectedDirectory.appendingPathComponent("devclean.jsonl")
        try? fileManager.createDirectory(
            at: selectedDirectory,
            withIntermediateDirectories: true
        )
    }

    public func write(
        level: String,
        event: String,
        properties: [String: String] = [:]
    ) {
        rotateIfNeeded()
        let record = LocalLogRecord(
            timestamp: ISO8601DateFormatter().string(from: Date()),
            level: level,
            event: event,
            properties: properties
        )
        guard let data = try? encoder.encode(record) else { return }
        var line = data
        line.append(0x0A)
          if !FileManager.default.fileExists(atPath: logURL.path) {
              FileManager.default.createFile(atPath: logURL.path, contents: line)
              try? FileManager.default.setAttributes(
                  [.posixPermissions: 0o600],
                  ofItemAtPath: logURL.path
              )
              return
          }
        guard let handle = try? FileHandle(forWritingTo: logURL) else { return }
        defer { try? handle.close() }
          do {
              try handle.seekToEnd()
              try handle.write(contentsOf: line)
              try? FileManager.default.setAttributes(
                  [.posixPermissions: 0o600],
                  ofItemAtPath: logURL.path
              )
          } catch {
            // A logging failure must never block scanning or cleanup.
        }
    }

    public func openDirectory() {
        NSWorkspace.shared.open(logDirectory)
    }

    private func rotateIfNeeded() {
        guard let size = try? logURL.resourceValues(forKeys: [.fileSizeKey]).fileSize,
              UInt64(size) >= maximumBytes
        else { return }
        let previous = logDirectory.appendingPathComponent("devclean.previous.jsonl")
        try? FileManager.default.removeItem(at: previous)
        try? FileManager.default.moveItem(at: logURL, to: previous)
    }
}

@MainActor
public final class MonitoringCenter: AnalyticsService {
    public static let shared = MonitoringCenter()

    private let logger: LocalDiagnosticsLogger
    private var sentryStarted = false
    private var sentryDSN: String?

    public init(logger: LocalDiagnosticsLogger = .shared) {
        self.logger = logger
    }

    public var isRemoteConfigured: Bool {
        guard let sentryDSN else { return false }
        return !sentryDSN.isEmpty
    }

    public func configure(
        defaults: UserDefaults = .standard,
        bundle: Bundle = .main,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) {
        sentryDSN = environment["DEVCLEAN_SENTRY_DSN"]
            ?? bundle.object(forInfoDictionaryKey: "DevcleanSentryDSN") as? String
        setRemoteConsent(defaults.bool(forKey: PreferenceKeys.anonymousDiagnostics))
        track(.appLaunched, properties: [
            "app_version": bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "dev",
            "os_major": String(ProcessInfo.processInfo.operatingSystemVersion.majorVersion),
        ])
    }

    public func setRemoteConsent(_ enabled: Bool) {
        guard enabled, let dsn = sentryDSN, !dsn.isEmpty else {
            if sentryStarted {
                SentrySDK.close()
                sentryStarted = false
            }
            return
        }
        guard !sentryStarted else { return }
        SentrySDK.start { options in
            options.dsn = dsn
            options.sendDefaultPii = false
            options.enableAutoSessionTracking = true
            options.enableAppHangTracking = true
            options.tracesSampleRate = 0.05
            options.environment = "production"
        }
        sentryStarted = true
    }

    public func track(
        _ event: AnalyticsEventName,
        properties: [String: String] = [:]
    ) {
          logger.write(level: "info", event: event.rawValue, properties: properties)
          guard sentryStarted, event == .learningSummary else { return }
          captureRemote(message: "devclean.learning_summary", properties: properties)
    }

    public func capture(error: Error, operation: String) {
        let fingerprint = Self.errorFingerprint(error)
        logger.write(
            level: "error",
            event: "operation_failed",
            properties: [
                "operation": operation,
                "error_type": fingerprint,
                "local_message": error.localizedDescription,
            ]
          )
          guard sentryStarted else { return }
          captureRemote(
              message: "devclean.operation_failed",
              properties: ["operation": operation, "error_type": fingerprint]
          )
    }

    public func openLocalLogs() {
        logger.openDirectory()
    }

      private static func errorFingerprint(_ error: Error) -> String {
        let nsError = error as NSError
        let domain = nsError.domain
            .replacingOccurrences(of: ".", with: "_")
            .replacingOccurrences(of: " ", with: "_")
            .lowercased()
          return "\(domain)_\(nsError.code)"
      }

      private func captureRemote(message: String, properties: [String: String]) {
          SentrySDK.capture(message: message) { scope in
              for (key, value) in properties {
                  scope.setTag(value: value, key: key)
              }
          }
      }
  }
