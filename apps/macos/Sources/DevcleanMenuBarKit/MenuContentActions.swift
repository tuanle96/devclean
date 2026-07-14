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

                if aiInsightsEnabled {
                    // The one manual entry point for AI recommendations; the
                    // monitoring banner above the list is the only other surface.
                    Button {
                        presentAIInsight()
                    } label: {
                        Image(systemName: "sparkles")
                    }
                    .help("AI recommendations")
                    .accessibilityLabel("AI recommendations")
                    .accessibilityIdentifier("ai-recommend-open")
                }

                Spacer()

                switch selectedSection {
                case .clean:
                    Button(cleanButtonTitle) {
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
                    // "Delete All" lives in the Holds summary menu, away from the
                    // window edge; nothing competes with Scan down here.
                    EmptyView()
                case .memory:
                    // Read-only monitor; termination stays out of Swift entirely.
                    EmptyView()
                }
            }
        }
        .controlSize(.large)
    }

    /// States the consequence on the button itself ("Clean 38.77 GB…"), matching
    /// the sizing pattern every destructive button in the app already follows.
    var cleanButtonTitle: String {
        model.selectedBytes > 0
            ? "Clean \(ByteFormatting.string(model.selectedBytes))…"
            : "Clean…"
    }

    var footer: some View {
        HStack {
            openSettingsButton {
                Label("Settings…", systemImage: "gear")
            }
            .buttonStyle(.plain)
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .keyboardShortcut("q")
                .help("Quit DevCleaner (⌘Q)")
        }
        .font(.caption)
    }

    /// Opens Settings via the supported `SettingsLink` on macOS 14+, falling back
    /// to the legacy selector only where no public API exists (macOS 13).
    @ViewBuilder
    func openSettingsButton<L: View>(@ViewBuilder label: () -> L) -> some View {
        if #available(macOS 14.0, *) {
            SettingsLink(label: label)
                .simultaneousGesture(
                    TapGesture().onEnded {
                        SettingsWindowFocusCoordinator.activateAfterSettingsLink()
                    }
                )
        } else {
            Button(action: openSettingsWindow, label: label)
        }
    }

    func openSettingsWindow() {
        NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
        SettingsWindowFocusCoordinator.activateWhenAvailable()
    }

    /// Viewport cap for the candidate lists. Rows size themselves; this only
    /// bounds how tall the scrollable area may grow inside the popover.
    func listHeight(for rowCount: Int, rowHeight: CGFloat) -> CGFloat {
        min(340, CGFloat(rowCount) * rowHeight)
    }
}
