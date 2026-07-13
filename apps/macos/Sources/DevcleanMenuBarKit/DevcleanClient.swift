import Foundation

public struct CommandResult: Equatable, Sendable {
    public let status: Int32
    public let standardOutput: String
    public let standardError: String

    public init(status: Int32, standardOutput: String, standardError: String) {
        self.status = status
        self.standardOutput = standardOutput
        self.standardError = standardError
    }
}

public protocol CommandExecuting: Sendable {
    func run(executable: URL, arguments: [String]) async throws -> CommandResult
}

/// Holds the running process so a cancelled parent Task can terminate it, which is
/// what lets the menu bar "Cancel" actually stop a long scan instead of leaving a
/// zombie helper running while the UI reports idle.
private final class ProcessHolder: @unchecked Sendable {
    private let lock = NSLock()
    private var process: Process?
    private var cancelled = false

    /// Stores the process and reports whether it may still be launched.
    func store(_ process: Process) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        self.process = process
        return !cancelled
    }

    func terminate() {
        lock.lock()
        defer { lock.unlock() }
        cancelled = true
        if let process, process.isRunning {
            process.terminate()
        }
    }
}

public struct FoundationCommandExecutor: CommandExecuting {
    public init() {}

    public func run(executable: URL, arguments: [String]) async throws -> CommandResult {
        let holder = ProcessHolder()
        return try await withTaskCancellationHandler {
            try await Task.detached(priority: .userInitiated) {
                let fileManager = FileManager.default
                let directory = fileManager.temporaryDirectory
                    .appendingPathComponent(
                        "devclean-process-\(UUID().uuidString)", isDirectory: true)
                try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
                defer { try? fileManager.removeItem(at: directory) }

                let stdoutURL = directory.appendingPathComponent("stdout")
                let stderrURL = directory.appendingPathComponent("stderr")
                fileManager.createFile(atPath: stdoutURL.path, contents: nil)
                fileManager.createFile(atPath: stderrURL.path, contents: nil)
                let stdout = try FileHandle(forWritingTo: stdoutURL)
                let stderr = try FileHandle(forWritingTo: stderrURL)
                defer {
                    try? stdout.close()
                    try? stderr.close()
                }

                let process = Process()
                process.executableURL = executable
                process.arguments = arguments
                process.standardOutput = stdout
                process.standardError = stderr
                guard holder.store(process) else { throw CancellationError() }
                try process.run()
                process.waitUntilExit()
                try stdout.synchronize()
                try stderr.synchronize()

                let outputData = try Data(contentsOf: stdoutURL)
                let errorData = try Data(contentsOf: stderrURL)
                return CommandResult(
                    status: process.terminationStatus,
                    standardOutput: String(decoding: outputData, as: UTF8.self),
                    standardError: String(decoding: errorData, as: UTF8.self)
                )
            }.value
        } onCancel: {
            holder.terminate()
        }
    }
}

public enum DevcleanArguments {
    public static func scan(
        settings: ScanSettings,
        approvedReviewPaths: [String] = []
    ) -> [String] {
        var arguments =
            ["scan"]
            + shared(
                settings: settings,
                approvedReviewPaths: approvedReviewPaths
            )
        if settings.learningMode {
            arguments.append("--learning")
        }
        return arguments + ["--format", "json"]
    }

    public static func clean(
        paths: [String],
        settings: ScanSettings,
        approvedReviewPaths: [String] = []
    ) -> [String] {
        var arguments =
            ["clean"]
            + shared(
                settings: settings,
                approvedReviewPaths: approvedReviewPaths
            )
        for path in paths.sorted() {
            arguments += ["--only-path", path]
        }
        if let quarantineFor = settings.quarantineFor {
            arguments += ["--quarantine-for", quarantineFor]
        }
        arguments.append("--yes")
        return arguments
    }

    public static let listQuarantine = ["quarantine", "list", "--json"]
    public static let purgeExpiredQuarantine = ["quarantine", "purge", "--json"]
    public static let purgeAllQuarantine = ["quarantine", "purge", "--all", "--json"]

    public static func purgeQuarantine(id: String) -> [String] {
        ["quarantine", "purge", "--id", id, "--json"]
    }

    public static func restoreQuarantine(id: String) -> [String] {
        ["quarantine", "restore", id]
    }

    private static func shared(
        settings: ScanSettings,
        approvedReviewPaths: [String]
    ) -> [String] {
        var arguments = settings.roots
        for category in settings.categories.sorted(by: { $0.rawValue < $1.rawValue }) {
            arguments += ["--category", category.rawValue]
        }
        if settings.includeGlobalCaches {
            arguments.append("--global-caches")
        }
        if settings.includeExpensiveCaches {
            arguments.append("--expensive-caches")
        }
        if let olderThan = settings.olderThan {
            arguments += ["--older-than", olderThan]
        }
        if let minimumSize = settings.minimumSize {
            arguments += ["--min-size", minimumSize]
        }
        for path in approvedReviewPaths.sorted() {
            arguments += ["--approve-review-path", path]
        }
        return arguments
    }
}

public enum DevcleanLocator {
    public static func locate(
        bundle: Bundle = .main,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
    ) -> URL? {
        var candidates: [URL] = []
        if let override = environment["DEVCLEAN_EXECUTABLE"], !override.isEmpty {
            candidates.append(URL(fileURLWithPath: override))
        }
        candidates.append(bundle.bundleURL.appendingPathComponent("Contents/Helpers/devclean"))
        candidates.append(homeDirectory.appendingPathComponent(".local/bin/devclean"))
        candidates.append(URL(fileURLWithPath: "/opt/homebrew/bin/devclean"))
        candidates.append(URL(fileURLWithPath: "/usr/local/bin/devclean"))
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }
}

public enum DevcleanClientError: LocalizedError, Equatable, Sendable {
    case executableNotFound
    case commandFailed(String)
    case invalidReport(String)

    public var errorDescription: String? {
        switch self {
        case .executableNotFound:
            "The bundled devclean helper could not be found."
        case .commandFailed(let message):
            message
        case .invalidReport(let message):
            "devclean returned an invalid scan report: \(message)"
        }
    }
}

public actor DevcleanClient {
    private let executable: URL?
    private let executor: any CommandExecuting

    public init(
        executable: URL? = DevcleanLocator.locate(),
        executor: any CommandExecuting = FoundationCommandExecutor()
    ) {
        self.executable = executable
        self.executor = executor
    }

    public func scan(
        settings: ScanSettings,
        approvedReviewPaths: [String] = []
    ) async throws -> ScanReport {
        let result = try await execute(
            DevcleanArguments.scan(
                settings: settings,
                approvedReviewPaths: approvedReviewPaths
            )
        )
        do {
            return try JSONDecoder().decode(ScanReport.self, from: Data(result.standardOutput.utf8))
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func clean(
        paths: [String],
        settings: ScanSettings,
        approvedReviewPaths: [String] = []
    ) async throws -> String {
        let result = try await execute(
            DevcleanArguments.clean(
                paths: paths,
                settings: settings,
                approvedReviewPaths: approvedReviewPaths
            )
        )
        return result.standardOutput.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    public func listQuarantine() async throws -> [QuarantineEntry] {
        let result = try await execute(DevcleanArguments.listQuarantine)
        do {
            return try JSONDecoder().decode(
                [QuarantineEntry].self,
                from: Data(result.standardOutput.utf8)
            )
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func purgeExpiredQuarantine() async throws -> QuarantinePurgeReport {
        let result = try await execute(DevcleanArguments.purgeExpiredQuarantine)
        do {
            return try JSONDecoder().decode(
                QuarantinePurgeReport.self,
                from: Data(result.standardOutput.utf8)
            )
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func purgeAllQuarantine() async throws -> QuarantinePurgeReport {
        let result = try await execute(DevcleanArguments.purgeAllQuarantine)
        do {
            return try JSONDecoder().decode(
                QuarantinePurgeReport.self,
                from: Data(result.standardOutput.utf8)
            )
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func purgeQuarantine(id: String) async throws -> QuarantinePurgeReport {
        let result = try await execute(DevcleanArguments.purgeQuarantine(id: id))
        do {
            return try JSONDecoder().decode(
                QuarantinePurgeReport.self,
                from: Data(result.standardOutput.utf8)
            )
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func restoreQuarantine(id: String) async throws {
        _ = try await execute(DevcleanArguments.restoreQuarantine(id: id))
    }

    private func execute(_ arguments: [String]) async throws -> CommandResult {
        guard let executable else {
            throw DevcleanClientError.executableNotFound
        }
        let result = try await executor.run(executable: executable, arguments: arguments)
        guard result.status == 0 else {
            let message = result.standardError.trimmingCharacters(in: .whitespacesAndNewlines)
            throw DevcleanClientError.commandFailed(
                message.isEmpty ? "devclean exited with status \(result.status)." : message
            )
        }
        return result
    }
}
