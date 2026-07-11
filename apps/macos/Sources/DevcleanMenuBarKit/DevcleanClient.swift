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

public struct FoundationCommandExecutor: CommandExecuting {
    public init() {}

    public func run(executable: URL, arguments: [String]) async throws -> CommandResult {
        try await Task.detached(priority: .userInitiated) {
            let fileManager = FileManager.default
            let directory = fileManager.temporaryDirectory
                .appendingPathComponent("devclean-process-\(UUID().uuidString)", isDirectory: true)
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
    }
}

public enum DevcleanArguments {
    public static func scan(settings: ScanSettings) -> [String] {
        ["scan"] + shared(settings: settings) + ["--format", "json"]
    }

    public static func clean(paths: [String], settings: ScanSettings) -> [String] {
        var arguments = ["clean"] + shared(settings: settings)
        for path in paths.sorted() {
            arguments += ["--only-path", path]
        }
        arguments.append("--yes")
        return arguments
    }

    private static func shared(settings: ScanSettings) -> [String] {
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
        case let .commandFailed(message):
            message
        case let .invalidReport(message):
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

    public func scan(settings: ScanSettings) async throws -> ScanReport {
        let result = try await execute(DevcleanArguments.scan(settings: settings))
        do {
            return try JSONDecoder().decode(ScanReport.self, from: Data(result.standardOutput.utf8))
        } catch {
            throw DevcleanClientError.invalidReport(error.localizedDescription)
        }
    }

    public func clean(paths: [String], settings: ScanSettings) async throws -> String {
        let result = try await execute(DevcleanArguments.clean(paths: paths, settings: settings))
        return result.standardOutput.trimmingCharacters(in: .whitespacesAndNewlines)
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
