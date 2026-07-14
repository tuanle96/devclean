import AppKit
import SwiftUI

extension MemoryPressure {
    var label: String {
        switch self {
        case .normal: return "Memory pressure normal"
        case .warning: return "Memory pressure elevated"
        case .critical: return "Memory pressure critical"
        case .unknown: return "Memory"
        }
    }

    var color: Color {
        switch self {
        case .normal: return .green
        case .warning: return .orange
        case .critical: return .red
        case .unknown: return .gray
        }
    }
}

extension MenuContentView {

    /// Samples while the menu window is open; the owning `.task` cancels when
    /// the window closes, so nothing runs in the background.
    func sampleMemoryWhileVisible() async {
        while !Task.isCancelled {
            memorySnapshot = await Task.detached(priority: .utility) {
                MemoryMonitor.sample()
            }.value
            try? await Task.sleep(nanoseconds: 5_000_000_000)
        }
    }

    @ViewBuilder
    var memorySummary: some View {
        if let snapshot = memorySnapshot {
            VStack(alignment: .leading, spacing: 7) {
                HStack(spacing: 6) {
                    Circle()
                        .fill(snapshot.pressure.color)
                        .frame(width: 8, height: 8)
                        .accessibilityHidden(true)
                    Text(snapshot.pressure.label)
                        .font(.subheadline.weight(.semibold))
                    Spacer()
                    Text(memoryUsedText(snapshot))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                // Pressure decides the color: the bar reads as "how worried to
                // be", not as a free-RAM figure to optimize.
                GeometryReader { geometry in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Color(nsColor: .separatorColor))
                        Capsule()
                            .fill(snapshot.pressure.color)
                            .frame(width: geometry.size.width * usedFraction(snapshot))
                    }
                }
                .frame(height: 6)
            }
            .padding(10)
            .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
            .accessibilityElement(children: .combine)
            .accessibilityLabel("\(snapshot.pressure.label). \(memoryUsedText(snapshot)).")
        }
    }

    @ViewBuilder
    var memoryContent: some View {
        if let snapshot = memorySnapshot {
            if snapshot.devProcesses.isEmpty {
                VStack(spacing: 9) {
                    Image(systemName: "checkmark.circle")
                        .font(.system(size: 28))
                        .foregroundStyle(.green)
                        .accessibilityHidden(true)
                    Text("No heavy dev tooling running")
                        .font(.headline)
                    Text("Daemons, simulators, and runtimes using significant memory will show up here.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 300)
                }
                .frame(maxWidth: .infinity, minHeight: 150)
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text(devToolingSummaryText(snapshot))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    List {
                        ForEach(snapshot.devProcesses) { process in
                            devProcessRow(process)
                                .listRowInsets(EdgeInsets(top: 2, leading: 4, bottom: 2, trailing: 4))
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .frame(height: listHeight(for: snapshot.devProcesses.count, rowHeight: 32))
                    Text("Read-only. These tools restart on demand the next time you build or run.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        } else {
            HStack(spacing: 10) {
                ProgressView()
                Text("Reading memory…")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 120)
        }
    }

    /// Leads with the owning project like the artifact rows do; the chip says
    /// what the process actually runs, which is what tells 20 `node`s apart.
    func devProcessRow(_ process: DevProcess) -> some View {
        HStack(spacing: 8) {
            Text(process.project ?? process.name)
                .font(.callout.weight(.medium))
                .lineLimit(1)
            MetaChip(text: devProcessChipText(process))
            Spacer(minLength: 8)
            Text(ByteFormatting.string(process.bytes))
                .font(.callout.monospacedDigit())
                .foregroundStyle(.secondary)
        }
        .help(devProcessHelp(process))
        .accessibilityElement(children: .combine)
    }

    func devProcessChipText(_ process: DevProcess) -> String {
        process.detail ?? (process.project == nil ? process.kind : process.name)
    }

    func devProcessHelp(_ process: DevProcess) -> String {
        var parts = ["PID \(process.pid)", process.name, process.kind]
        if let cwd = process.workingDirectory { parts.append(cwd) }
        return parts.joined(separator: " · ")
    }

    func memoryUsedText(_ snapshot: MemorySnapshot) -> String {
        "\(ByteFormatting.string(snapshot.usedBytes)) of \(ByteFormatting.string(snapshot.totalBytes)) used"
    }

    func devToolingSummaryText(_ snapshot: MemorySnapshot) -> String {
        let count = snapshot.devProcesses.count
        let noun = count == 1 ? "dev tool" : "dev tools"
        return "\(count) \(noun) holding \(ByteFormatting.string(snapshot.devBytes))"
    }

    func usedFraction(_ snapshot: MemorySnapshot) -> CGFloat {
        guard snapshot.totalBytes > 0 else { return 0 }
        return min(1, CGFloat(Double(snapshot.usedBytes) / Double(snapshot.totalBytes)))
    }
}
