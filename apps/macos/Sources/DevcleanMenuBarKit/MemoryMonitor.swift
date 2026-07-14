import Darwin
import Foundation

/// How constrained the system is, straight from the kernel's own signal
/// (`kern.memorystatus_vm_pressure_level`). Pressure — not "free" RAM — is
/// Apple's supported indicator of whether memory is actually tight.
public enum MemoryPressure: Sendable {
    case normal
    case warning
    case critical
    case unknown
}

/// Read-only snapshot of system memory plus the restartable dev tooling using it.
public struct MemorySnapshot: Sendable {
    public let totalBytes: UInt64
    public let usedBytes: UInt64
    public let pressure: MemoryPressure
    /// Largest footprint first; only same-user processes at or above
    /// `DevProcessSampler.footprintThreshold`.
    public let devProcesses: [DevProcess]

    public var devBytes: UInt64 {
        devProcesses.reduce(0) { $0 + $1.bytes }
    }
}

/// Samples system memory and enumerates developer tooling. Read-only by design:
/// nothing here signals, kills, or purges. Mirroring the scan/clean split,
/// display stays in Swift while any future termination authority would live in
/// the Rust CLI with its own revalidation.
public enum MemoryMonitor {
    /// Cached host port: `mach_host_self()` allocates a send right per call and
    /// this samples every few seconds while the menu is open.
    private static let machHost = mach_host_self()

    public static func sample() -> MemorySnapshot {
        MemorySnapshot(
            totalBytes: ProcessInfo.processInfo.physicalMemory,
            usedBytes: usedBytes() ?? 0,
            pressure: pressureLevel(),
            devProcesses: DevProcessSampler.processes()
        )
    }

    /// Active + wired + compressed pages — memory the kernel cannot drop without
    /// swapping. File cache is deliberately excluded: it is reclaimed on demand,
    /// and counting it is how "RAM cleaners" invent work for themselves.
    private static func usedBytes() -> UInt64? {
        var stats = vm_statistics64_data_t()
        var count = mach_msg_type_number_t(
            MemoryLayout<vm_statistics64_data_t>.size / MemoryLayout<integer_t>.size
        )
        let result = withUnsafeMutablePointer(to: &stats) { pointer in
            pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                host_statistics64(machHost, HOST_VM_INFO64, $0, &count)
            }
        }
        guard result == KERN_SUCCESS else { return nil }
        let pages =
            UInt64(stats.active_count) + UInt64(stats.wire_count)
            + UInt64(stats.compressor_page_count)
        return pages * UInt64(sysconf(_SC_PAGESIZE))
    }

    private static func pressureLevel() -> MemoryPressure {
        var level: Int32 = 0
        var size = MemoryLayout<Int32>.size
        guard sysctlbyname("kern.memorystatus_vm_pressure_level", &level, &size, nil, 0) == 0 else {
            return .unknown
        }
        // DISPATCH_MEMORYPRESSURE_{NORMAL,WARN,CRITICAL} levels.
        switch level {
        case 1: return .normal
        case 2: return .warning
        case 4: return .critical
        default: return .unknown
        }
    }
}
