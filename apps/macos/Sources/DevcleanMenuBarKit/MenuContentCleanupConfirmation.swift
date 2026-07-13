import AppKit
import SwiftUI

extension MenuContentView {

    @ViewBuilder
    var cleanupConfirmationContent: some View {
        switch cleanupConfirmation.stage {
        case .hidden:
            EmptyView()
        case .chooseMethod:
            Text(cleanupTitle)
                .font(.headline)
                .accessibilityFocused($overlayFocus, equals: .cleanupChoose)
                .onAppear { overlayFocus = .cleanupChoose }

            Text(
                model.usesSafetyHold
                    ? "Keep a restorable \(model.safetyHoldDays)-day safety hold, or permanently delete now to reclaim the space immediately. DevCleaner re-verifies every path either way."
                    : "Safety holds are disabled. Continue to a final confirmation before permanent deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            // Mac alert anatomy: destructive alternative leading, Cancel beside the
            // safe default action trailing; Return holds, Esc cancels.
            HStack(spacing: 8) {
                Button("Delete Now…", role: .destructive) {
                    cleanupConfirmation.requestImmediateDeletion()
                }
                .disabled(model.isBusy || model.selectedPaths.isEmpty)
                .accessibilityIdentifier("cleanup-delete-now")

                Spacer()

                cleanupCancelButton

                if model.usesSafetyHold {
                    Button(
                        "Hold for \(model.safetyHoldDays) \(model.safetyHoldDays == 1 ? "Day" : "Days")"
                    ) {
                        savedSelection = nil
                        cleanupConfirmation.confirm {
                            model.cleanSelected(disposition: .configuredSafetyHold)
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .keyboardShortcut(.defaultAction)
                    .disabled(model.isBusy || model.selectedPaths.isEmpty)
                    .accessibilityIdentifier("cleanup-hold")
                }
            }
        case .confirmImmediateDeletion:
            Text("Permanently delete \(ByteFormatting.string(model.selectedBytes))?")
                .font(.headline)
                .accessibilityFocused($overlayFocus, equals: .cleanupConfirm)
                .onAppear { overlayFocus = .cleanupConfirm }

            Text(
                "This reclaims the space immediately and cannot be undone or restored. DevCleaner re-verifies every selected path right before deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            HStack(spacing: 8) {
                Spacer()

                cleanupCancelButton

                // Visually primary because deletion is this step's only purpose;
                // Return deliberately stays unbound for a destructive action.
                Button(role: .destructive) {
                    savedSelection = nil
                    cleanupConfirmation.confirm {
                        model.cleanSelected(disposition: .deleteImmediately)
                    }
                } label: {
                    Text("Permanently Delete \(ByteFormatting.string(model.selectedBytes))")
                }
                .buttonStyle(.borderedProminent)
                .tint(.red)
                .disabled(model.isBusy || model.selectedPaths.isEmpty)
                .accessibilityIdentifier("cleanup-delete-confirm")
            }
        }
    }

    var cleanupTitle: String {
        let count = model.selectedPaths.count
        return
            "Clean \(count) \(count == 1 ? "item" : "items") (\(ByteFormatting.string(model.selectedBytes)))?"
    }

    var cleanupCancelButton: some View {
        Button("Cancel", role: .cancel) {
            cancelCleanup()
        }
        .keyboardShortcut(.cancelAction)
        .accessibilityIdentifier("cleanup-cancel")
    }

    /// Dismisses the cleanup confirmation and restores any selection an AI
    /// recommendation temporarily narrowed.
    func cancelCleanup() {
        cleanupConfirmation.cancel()
        if let saved = savedSelection {
            model.setSelection(saved)
            savedSelection = nil
        }
    }
}
