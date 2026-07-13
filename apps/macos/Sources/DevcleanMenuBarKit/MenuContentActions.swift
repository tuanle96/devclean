import AppKit
import SwiftUI

extension MenuContentView {

    var actions: some View {
        HStack(spacing: 8) {
            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
                Text(model.activityDescription ?? "Working…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("activity-progress")
                Spacer()
                if model.phase == .scanning {
                    Button("Cancel") { model.cancelScan() }
                        .controlSize(.small)
                        .keyboardShortcut(.cancelAction)
                        .accessibilityIdentifier("cancel-scan")
                }
            } else {
                Button {
                    model.scan()
                } label: {
                    Label("Scan", systemImage: "arrow.clockwise")
                }
                .keyboardShortcut("r")
                .help("Scan for artifacts again (⌘R)")

                Spacer()

                switch selectedSection {
                case .clean:
                    Button("Clean Selected") {
                        cleanupConfirmation.present()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(model.selectedPaths.isEmpty)
                    .accessibilityIdentifier("clean-selected")
                case .review:
                    Text("Approvals apply only to the folders you approve")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                case .holds:
                    if !model.quarantineEntries.isEmpty {
                        Button(role: .destructive) {
                            holdPurgeRequest = .all
                        } label: {
                            Label("Delete All…", systemImage: "trash")
                        }
                        .buttonStyle(.bordered)
                        .accessibilityIdentifier("hold-delete-all")
                    }
                }
            }
        }
        .controlSize(.large)
    }

    var footer: some View {
        HStack {
            if #available(macOS 14.0, *) {
                SettingsLink {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
                .simultaneousGesture(
                    TapGesture().onEnded {
                        SettingsWindowFocusCoordinator.activateAfterSettingsLink()
                    }
                )
            } else {
                Button {
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                    SettingsWindowFocusCoordinator.activateWhenAvailable()
                } label: {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .keyboardShortcut("q")
                .help("Quit DevCleaner (⌘Q)")
        }
        .font(.caption)
    }

    func openSettingsWindow() {
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
        SettingsWindowFocusCoordinator.activateWhenAvailable()
    }

    func listHeight(for rowCount: Int, rowHeight: CGFloat) -> CGFloat {
        min(340, max(100, CGFloat(rowCount) * rowHeight))
    }
}
