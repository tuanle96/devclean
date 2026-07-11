import Foundation
import Testing
@testable import DevcleanMenuBarKit

@Test
func scanReportDecodesRustJSONContract() throws {
    let json = #"""
    {
      "roots": ["/Users/me/Dev"],
      "candidates": [{
        "category": "rust-target",
        "path": "/Users/me/Dev/app/target",
        "bytes": 1048576,
        "reason": "Cargo target with build markers",
        "modified_at_unix": 1234
      }],
      "warnings": [],
      "total_bytes": 1048576,
      "protect_git_tracked": true
    }
    """#

    let report = try JSONDecoder().decode(ScanReport.self, from: Data(json.utf8))

    #expect(report.candidates.first?.category == .rustTarget)
    #expect(report.totalBytes == 1_048_576)
    #expect(report.protectGitTracked)
}

@Test
func scanArgumentsUseMachineReadableOutputAndConservativeCategories() {
    let settings = ScanSettings(roots: ["~/Dev"])

    let arguments = DevcleanArguments.scan(settings: settings)

    #expect(arguments.first == "scan")
    #expect(arguments.suffix(2) == ["--format", "json"])
    #expect(arguments.contains("rust-target"))
    #expect(arguments.contains("node-modules"))
    #expect(arguments.contains("--learning"))
    #expect(!arguments.contains("--allow-tracked"))
}

@Test
func cleanArgumentsContainOnlyExactPathsAndNonInteractiveConfirmation() {
    let settings = ScanSettings(roots: ["/Users/me/Dev"])

    let arguments = DevcleanArguments.clean(
        paths: ["/Users/me/Dev/b/node_modules", "/Users/me/Dev/a/target"],
        settings: settings
    )

    #expect(arguments.first == "clean")
    #expect(arguments.filter { $0 == "--only-path" }.count == 2)
    #expect(arguments.last == "--yes")
    #expect(arguments.contains("--quarantine-for"))
    #expect(arguments.contains("7d"))
    #expect(!arguments.contains("--allow-tracked"))
    #expect(!arguments.contains("--docker-system"))
}

@Test
func locatorPrefersExplicitExecutableOverride() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-locator-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let executable = directory.appendingPathComponent("devclean")
    FileManager.default.createFile(atPath: executable.path, contents: Data())
    try FileManager.default.setAttributes(
        [.posixPermissions: 0o755],
        ofItemAtPath: executable.path
    )

    let located = DevcleanLocator.locate(
        environment: ["DEVCLEAN_EXECUTABLE": executable.path],
        homeDirectory: directory
    )

    #expect(located == executable)
}

@MainActor
@Test
func learningStoreTracksGrowthAndReviewCandidatesAcrossDays() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-learning-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let store = LearningStore(stateURL: directory.appendingPathComponent("learning.json"))
    let first = try decodeReport(safeBytes: 1_000_000_000, reviewBytes: nil)
    let second = try decodeReport(safeBytes: 2_000_000_000, reviewBytes: 500_000_000)

    _ = try store.record(report: first, at: Date(timeIntervalSince1970: 1_700_000_000))
    let summary = try store.record(
        report: second,
        at: Date(timeIntervalSince1970: 1_700_086_400)
    )

    #expect(summary.observedDays == 2)
    #expect(summary.reviewCount == 1)
    #expect(summary.growthBytes == 1_500_000_000)
}

@MainActor
@Test
func learningFeedbackPersistsWithoutLeavingLocalStore() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-feedback-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: directory) }
    let stateURL = directory.appendingPathComponent("learning.json")
    let first = LearningStore(stateURL: stateURL)
    try first.recordFeedback(.neverClean, path: "/private/project/cache")

    let reopened = LearningStore(stateURL: stateURL)

    #expect(reopened.feedback(for: "/private/project/cache") == .neverClean)
}

@MainActor
@Test
func localDiagnosticsWritesStructuredJSONLine() throws {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent("devclean-logs-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: directory) }
    let logger = LocalDiagnosticsLogger(logDirectory: directory)

    logger.write(level: "info", event: "scan_completed", properties: ["safe_count": "2"])

    let data = try Data(contentsOf: directory.appendingPathComponent("devclean.jsonl"))
    let text = String(decoding: data, as: UTF8.self)
    #expect(text.contains("\"event\":\"scan_completed\""))
    #expect(text.contains("\"safe_count\":\"2\""))
}

private func decodeReport(safeBytes: UInt64, reviewBytes: UInt64?) throws -> ScanReport {
    let review = reviewBytes.map { bytes in
        #"""
        [{
          "path": "/private/project/dist",
          "bytes": \#(bytes),
          "reason": "cache-like directory",
          "modified_at_unix": 1234,
          "confidence": "review"
        }]
        """#
    } ?? "[]"
    let json = #"""
    {
      "roots": ["/private/project"],
      "candidates": [{
        "category": "rust-target",
        "path": "/private/project/target",
        "bytes": \#(safeBytes),
        "reason": "Cargo target",
        "modified_at_unix": 1234,
        "confidence": "safe"
      }],
      "review_candidates": \#(review),
      "warnings": [],
      "total_bytes": \#(safeBytes),
      "review_total_bytes": \#(reviewBytes ?? 0),
      "protect_git_tracked": true
    }
    """#
    return try JSONDecoder().decode(ScanReport.self, from: Data(json.utf8))
}
