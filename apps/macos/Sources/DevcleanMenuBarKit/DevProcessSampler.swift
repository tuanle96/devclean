import Darwin
import Foundation

/// One developer-tooling process and the physical footprint it currently holds.
/// Footprint is `ri_phys_footprint`, the same figure Activity Monitor shows.
public struct DevProcess: Identifiable, Sendable {
    public let pid: pid_t
    public let name: String
    public let kind: String
    /// What the runtime is actually running ("vite.js", "Gradle daemon"), from
    /// its arguments — this is what tells 20 `node` rows apart.
    public let detail: String?
    /// Owning project, from the process working directory.
    public let project: String?
    public let workingDirectory: String?
    public let bytes: UInt64

    public var id: pid_t { pid }
}

/// Read-only libproc enumeration of restartable dev tooling. Every call here is
/// an inspection; failures (zombies, other users' processes) are skipped.
enum DevProcessSampler {
    /// Ignore processes below this footprint; idle helpers under it are noise.
    static let footprintThreshold: UInt64 = 64 * 1024 * 1024

    static func processes() -> [DevProcess] {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return listAllPids()
            .compactMap { pid -> DevProcess? in
                guard let path = executablePath(pid),
                    let kind = classify(executablePath: path),
                    let bytes = physicalFootprint(pid),
                    bytes >= footprintThreshold
                else { return nil }
                let name = URL(fileURLWithPath: path).lastPathComponent
                let cwd = workingDirectory(pid)
                return DevProcess(
                    pid: pid,
                    name: name,
                    kind: kind,
                    detail: detail(command: name, arguments: arguments(pid) ?? []),
                    project: cwd.flatMap { projectName(fromWorkingDirectory: $0, home: home) },
                    workingDirectory: cwd,
                    bytes: bytes
                )
            }
            .sorted { $0.bytes > $1.bytes }
    }

    // MARK: - Classification

    /// Exact executable names for restartable dev tooling. Data, not code —
    /// extending coverage is a one-line change.
    private static let kindsByName: [String: String] = [
        "node": "JavaScript runtime",
        "bun": "JavaScript runtime",
        "deno": "JavaScript runtime",
        "java": "JVM daemon",
        "rust-analyzer": "Language server",
        "gopls": "Language server",
        "clangd": "Language server",
        "sourcekit-lsp": "Language server",
        "jdtls": "Language server",
        "SourceKitService": "Xcode indexing",
        "swift-frontend": "Swift compiler",
        "esbuild": "Bundler",
        "watchman": "File watcher",
        "postgres": "Local database",
        "mysqld": "Local database",
        "redis-server": "Local database",
        "mongod": "Local database",
        "dockerd": "Container daemon",
        "containerd": "Container daemon",
        "buildkitd": "Container daemon",
        "vpnkit": "Virtual machine",
        "limactl": "Virtual machine",
        "colima": "Virtual machine",
        "Simulator": "iOS Simulator",
        "launchd_sim": "iOS Simulator",
    ]

    /// Path fragments for tooling whose process names vary (simulator services,
    /// Docker Desktop helpers). Markers are directory components, not name
    /// prefixes, so a user's "CoreSimulatorTools" project never matches.
    private static let kindsByPathMarker: [(marker: String, kind: String)] = [
        ("/CoreSimulator/", "iOS Simulator"),
        ("/CoreSimulator.framework/", "iOS Simulator"),
        ("/Docker.app/", "Docker Desktop"),
    ]

    /// The kind label for a dev-tooling executable, or nil for everything else.
    /// GUI apps the user can already see in the Dock are deliberately not listed.
    static func classify(executablePath path: String) -> String? {
        let name = URL(fileURLWithPath: path).lastPathComponent
        if let kind = kindsByName[name] { return kind }
        if name.hasPrefix("qemu-system") { return "Virtual machine" }
        return kindsByPathMarker.first { path.contains($0.marker) }?.kind
    }

    /// Human hint for what a generic runtime is running: JVMs are matched on
    /// daemon markers, everything else on the first script-like argument.
    static func detail(command: String, arguments: [String]) -> String? {
        if command == "java" {
            let joined = arguments.joined(separator: " ").lowercased()
            if joined.contains("gradle") { return "Gradle daemon" }
            if joined.contains("kotlin") { return "Kotlin daemon" }
            return nil
        }
        guard let entry = arguments.dropFirst().first(where: { !$0.hasPrefix("-") }) else {
            return nil
        }
        if entry.contains("claude-code") { return "Claude Code" }
        let base = URL(fileURLWithPath: entry).lastPathComponent
        return base.isEmpty || base == command ? nil : base
    }

    /// The working directory's basename names the owning project — the same
    /// lead-with-project convention the artifact rows follow. Home and root are
    /// not projects.
    static func projectName(fromWorkingDirectory cwd: String, home: String) -> String? {
        guard cwd != "/", cwd != home else { return nil }
        let name = URL(fileURLWithPath: cwd).lastPathComponent
        return name.isEmpty ? nil : name
    }

    // MARK: - libproc / sysctl reads

    private static func listAllPids() -> [pid_t] {
        let expected = proc_listallpids(nil, 0)
        guard expected > 0 else { return [] }
        // Headroom for processes spawned between the count and fill calls.
        var pids = [pid_t](repeating: 0, count: Int(expected) + 32)
        let filled = pids.withUnsafeMutableBufferPointer {
            proc_listallpids($0.baseAddress, Int32($0.count * MemoryLayout<pid_t>.size))
        }
        guard filled > 0 else { return [] }
        return Array(pids.prefix(Int(filled)))
    }

    private static func executablePath(_ pid: pid_t) -> String? {
        var buffer = [CChar](repeating: 0, count: 4 * 1024)
        let length = buffer.withUnsafeMutableBufferPointer {
            proc_pidpath(pid, $0.baseAddress, UInt32($0.count))
        }
        guard length > 0 else { return nil }
        let bytes = buffer.prefix(Int(length)).map { UInt8(bitPattern: $0) }
        return String(decoding: bytes, as: UTF8.self)
    }

    /// `ri_phys_footprint` — the figure Activity Monitor's Memory column shows.
    /// Pinned to V4 rather than RUSAGE_INFO_CURRENT: CURRENT is baked in by the
    /// build SDK, and a future bump would EINVAL on the older kernels this app
    /// still targets. V4 has the footprint and every macOS 13+ kernel serves it.
    private static func physicalFootprint(_ pid: pid_t) -> UInt64? {
        var info = rusage_info_v4()
        let result = withUnsafeMutablePointer(to: &info) { pointer in
            pointer.withMemoryRebound(to: (rusage_info_t?).self, capacity: 1) {
                proc_pid_rusage(pid, RUSAGE_INFO_V4, $0)
            }
        }
        guard result == 0 else { return nil }
        return info.ri_phys_footprint
    }

    private static func workingDirectory(_ pid: pid_t) -> String? {
        var info = proc_vnodepathinfo()
        let size = Int32(MemoryLayout<proc_vnodepathinfo>.size)
        guard proc_pidinfo(pid, PROC_PIDVNODEPATHINFO, 0, &info, size) > 0 else { return nil }
        return withUnsafeBytes(of: info.pvi_cdir.vip_path) { raw in
            let path = String(decoding: raw.prefix(while: { $0 != 0 }), as: UTF8.self)
            return path.isEmpty ? nil : path
        }
    }

    /// argv via `KERN_PROCARGS2`: an argc word, the exec path, NUL padding, then
    /// argc NUL-separated argument strings. Same-user processes only.
    private static func arguments(_ pid: pid_t) -> [String]? {
        var mib: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = 0
        guard sysctl(&mib, 3, nil, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else {
            return nil
        }
        var buffer = [UInt8](repeating: 0, count: size)
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0 else { return nil }
        let argc = buffer.withUnsafeBytes { $0.loadUnaligned(as: Int32.self) }
        guard argc > 0 else { return nil }
        var offset = MemoryLayout<Int32>.size
        while offset < size, buffer[offset] != 0 { offset += 1 }
        while offset < size, buffer[offset] == 0 { offset += 1 }
        var args: [String] = []
        var current: [UInt8] = []
        while offset < size, args.count < Int(argc) {
            if buffer[offset] == 0 {
                args.append(String(decoding: current, as: UTF8.self))
                current = []
            } else {
                current.append(buffer[offset])
            }
            offset += 1
        }
        return args
    }
}
