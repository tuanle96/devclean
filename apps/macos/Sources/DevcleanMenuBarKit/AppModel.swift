import Combine
import Foundation

public enum AppPhase: Equatable, Sendable {
    case idle
    case scanning
    case cleaning
}

@MainActor
public final class AppModel: ObservableObject {
    @Published public private(set) var phase: AppPhase = .idle
    @Published public private(set) var report: ScanReport?
    @Published public private(set) var selectedPaths: Set<String> = []
    @Published public private(set) var availableBytes: UInt64?
    @Published public private(set) var errorMessage: String?
    @Published public private(set) var statusMessage: String?

    private let client: DevcleanClient
    private let defaults: UserDefaults

    public init(
        client: DevcleanClient = DevcleanClient(),
        defaults: UserDefaults = .standard
    ) {
        self.client = client
        self.defaults = defaults
        refreshAvailableSpace()
    }

    public var isBusy: Bool { phase != .idle }

    public var selectedBytes: UInt64 {
        report?.candidates
            .filter { selectedPaths.contains($0.path) }
            .reduce(0) { $0 + $1.bytes } ?? 0
    }

    public var menuBarSymbol: String {
        switch phase {
        case .scanning, .cleaning: "arrow.triangle.2.circlepath"
        case .idle where (report?.totalBytes ?? 0) > 0: "externaldrive.badge.minus"
        case .idle: "externaldrive.badge.checkmark"
        }
    }

    public func initialLoad() {
        refreshAvailableSpace()
        if report == nil, !isBusy {
            scan()
        }
    }

    public func scan() {
        guard !isBusy else { return }
        phase = .scanning
        errorMessage = nil
        statusMessage = nil
        let settings = ScanSettings.load(from: defaults)
        Task {
            defer { phase = .idle }
            do {
                let report = try await client.scan(settings: settings)
                self.report = report
                selectedPaths = Set(report.candidates.map(\.path))
                statusMessage = report.candidates.isEmpty
                    ? "No eligible artifacts found."
                    : "Found \(report.candidates.count) rebuildable artifacts."
                refreshAvailableSpace()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    public func cleanSelected() {
        guard !isBusy, !selectedPaths.isEmpty else { return }
        phase = .cleaning
        errorMessage = nil
        statusMessage = nil
        let settings = ScanSettings.load(from: defaults)
        let paths = Array(selectedPaths)
        let bytesToReclaim = selectedBytes
        Task {
            defer { phase = .idle }
            do {
                _ = try await client.clean(paths: paths, settings: settings)
                let refreshed = try await client.scan(settings: settings)
                report = refreshed
                selectedPaths = Set(refreshed.candidates.map(\.path))
                statusMessage = "Cleanup completed. Reclaimed about \(ByteFormatting.string(bytesToReclaim))."
                refreshAvailableSpace()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    public func setSelected(_ selected: Bool, candidate: CleanupCandidate) {
        if selected {
            selectedPaths.insert(candidate.path)
        } else {
            selectedPaths.remove(candidate.path)
        }
    }

    public func isSelected(_ candidate: CleanupCandidate) -> Bool {
        selectedPaths.contains(candidate.path)
    }

    public func selectAll() {
        selectedPaths = Set(report?.candidates.map(\.path) ?? [])
    }

    public func selectNone() {
        selectedPaths.removeAll()
    }

    public func refreshAvailableSpace() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        availableBytes = try? home.resourceValues(
            forKeys: [.volumeAvailableCapacityForImportantUsageKey]
        ).volumeAvailableCapacityForImportantUsage.map(UInt64.init)
    }
}
