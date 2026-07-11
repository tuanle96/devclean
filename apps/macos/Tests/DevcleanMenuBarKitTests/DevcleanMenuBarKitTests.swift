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
