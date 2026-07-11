import AppKit
import SwiftUI

public struct MenuContentView: View {
    @ObservedObject private var model: AppModel
    @State private var showingConfirmation = false

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            Divider()
            content
            messages
            Divider()
            actions
            footer
        }
        .padding(16)
        .frame(width: 380)
        .confirmationDialog(
            "Clean selected artifacts?",
            isPresented: $showingConfirmation,
            titleVisibility: .visible
        ) {
            Button(
                "Clean \(ByteFormatting.string(model.selectedBytes))",
                role: .destructive
            ) {
                model.cleanSelected()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Rust will scan and validate every selected path again before quarantine and deletion.")
        }
        .task { model.initialLoad() }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Devclean")
                    .font(.headline)
                Text(freeSpaceText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 3) {
                Text(ByteFormatting.string(model.report?.totalBytes ?? 0))
                    .font(.headline.monospacedDigit())
                Text("reclaimable")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if model.phase == .scanning, model.report == nil {
            HStack(spacing: 10) {
                ProgressView()
                Text("Scanning development artifacts…")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 120)
        } else if let candidates = model.report?.candidates, !candidates.isEmpty {
            HStack {
                Text("Candidates")
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Button("All") { model.selectAll() }
                    .buttonStyle(.plain)
                Text("·").foregroundStyle(.tertiary)
                Button("None") { model.selectNone() }
                    .buttonStyle(.plain)
            }
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(candidates) { candidate in
                        candidateRow(candidate)
                        if candidate.id != candidates.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(maxHeight: 260)
        } else {
            VStack(spacing: 8) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("Disk looks clean")
                    .font(.headline)
                Text("No rebuildable artifacts match the current filters.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity, minHeight: 140)
        }
    }

    private func candidateRow(_ candidate: CleanupCandidate) -> some View {
        Toggle(
            isOn: Binding(
                get: { model.isSelected(candidate) },
                set: { model.setSelected($0, candidate: candidate) }
            )
        ) {
            HStack(spacing: 10) {
                Image(systemName: candidate.category.systemImage)
                    .frame(width: 18)
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(candidate.category.title)
                            .font(.subheadline)
                        Spacer()
                        Text(ByteFormatting.string(candidate.bytes))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    Text(candidate.path)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .help(candidate.path)
                }
            }
            .frame(minHeight: 44)
        }
        .toggleStyle(.checkbox)
        .accessibilityLabel("\(candidate.category.title), \(ByteFormatting.string(candidate.bytes))")
        .accessibilityHint(candidate.path)
    }

    @ViewBuilder
    private var messages: some View {
        if let error = model.errorMessage {
            Label(error, systemImage: "exclamationmark.triangle.fill")
                .font(.caption)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
        } else if let status = model.statusMessage {
            Text(status)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        if let warnings = model.report?.warnings, !warnings.isEmpty {
            Label("\(warnings.count) paths were skipped safely", systemImage: "info.circle")
                .font(.caption)
                .foregroundStyle(.secondary)
                .help(warnings.joined(separator: "\n"))
        }
    }

    private var actions: some View {
        HStack(spacing: 8) {
            Button {
                model.scan()
            } label: {
                Label("Scan", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r")
            .disabled(model.isBusy)

            Spacer()

            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
            }
            Button {
                showingConfirmation = true
            } label: {
                Label("Clean Selected", systemImage: "trash")
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
        }
        .controlSize(.large)
    }

    private var footer: some View {
        HStack {
            if #available(macOS 14.0, *) {
                SettingsLink {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            } else {
                Button {
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                } label: {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .keyboardShortcut("q")
        }
        .font(.caption)
    }

    private var freeSpaceText: String {
        guard let bytes = model.availableBytes else { return "Free space unavailable" }
        return "\(ByteFormatting.string(bytes)) free"
    }
}

public struct SettingsView: View {
    @AppStorage(PreferenceKeys.roots) private var roots = ""
    @AppStorage(PreferenceKeys.olderThan) private var olderThan = "7d"
    @AppStorage(PreferenceKeys.minimumSize) private var minimumSize = "100MiB"
    @AppStorage(PreferenceKeys.buildOutputs) private var buildOutputs = false
    @AppStorage(PreferenceKeys.testCaches) private var testCaches = false
    @AppStorage(PreferenceKeys.globalCaches) private var globalCaches = false
    @AppStorage(PreferenceKeys.expensiveCaches) private var expensiveCaches = false

    public init() {}

    public var body: some View {
        Form {
            Section("Scan roots") {
                TextEditor(text: $roots)
                    .font(.body.monospaced())
                    .frame(minHeight: 88)
                    .overlay(alignment: .topLeading) {
                        if roots.isEmpty {
                            Text("Leave empty for ~/Dev, ~/Projects and ~/Documents/Codex")
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 5)
                                .padding(.vertical, 8)
                                .allowsHitTesting(false)
                        }
                    }
                Text("Enter one directory per line. Tilde paths are supported.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Filters") {
                TextField("Older than", text: $olderThan, prompt: Text("7d"))
                TextField("Minimum size", text: $minimumSize, prompt: Text("100MiB"))
                Text("Leave either field empty to disable that filter.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Additional categories") {
                Toggle("Build outputs", isOn: $buildOutputs)
                Toggle("Test caches", isOn: $testCaches)
                Toggle("Package and tool caches", isOn: $globalCaches)
                Toggle("Runtime and model caches", isOn: $expensiveCaches)
                Text("Runtime and model caches can be expensive to download again.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Safety") {
                Label("Git-tracked files remain protected", systemImage: "lock.shield")
                Label("Docker volumes are never touched", systemImage: "externaldrive.badge.xmark")
                Label("Every clean performs a fresh Rust safety scan", systemImage: "checkmark.shield")
            }
        }
        .formStyle(.grouped)
        .frame(width: 500, height: 520)
    }
}
