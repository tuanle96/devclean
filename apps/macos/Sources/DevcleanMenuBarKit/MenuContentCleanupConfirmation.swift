import AppKit
import SwiftUI

extension MenuContentView {

    @ViewBuilder
    var cleanupConfirmationContent: some View {
        switch cleanupConfirmation.stage {
        case .hidden:
            EmptyView()
        case .chooseMethod:
            Text("Clean selected artifacts?")
                .font(.headline)

            Text(
                model.usesSafetyHold
                    ? "Keep a restorable \(model.safetyHoldDays)-day safety hold, or permanently delete now to reclaim the space immediately. DevCleaner re-verifies every path either way."
                    : "Safety holds are disabled. Continue to a final confirmation before permanent deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            if model.usesSafetyHold {
                Button {
                    savedSelection = nil
                    cleanupConfirmation.confirm {
                        model.cleanSelected(disposition: .configuredSafetyHold)
                    }
                } label: {
                    Text(
                        "Hold \(ByteFormatting.string(model.selectedBytes)) for \(model.safetyHoldDays) \(model.safetyHoldDays == 1 ? "Day" : "Days")"
                    )
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(model.isBusy || model.selectedPaths.isEmpty)
                .accessibilityIdentifier("cleanup-hold")
            }

            Button(role: .destructive) {
                cleanupConfirmation.requestImmediateDeletion()
            } label: {
                Text("Delete \(ByteFormatting.string(model.selectedBytes)) Now…")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
            .accessibilityIdentifier("cleanup-delete-now")

            cleanupCancelButton
        case .confirmImmediateDeletion:
            Text("Permanently delete selected artifacts?")
                .font(.headline)

            Text(
                "This reclaims the space immediately and cannot be undone or restored. DevCleaner re-verifies every selected path right before deletion."
            )
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            Button(role: .destructive) {
                savedSelection = nil
                cleanupConfirmation.confirm {
                    model.cleanSelected(disposition: .deleteImmediately)
                }
            } label: {
                Text("Permanently Delete \(ByteFormatting.string(model.selectedBytes))")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
            .accessibilityIdentifier("cleanup-delete-confirm")

            Button {
                cleanupConfirmation.returnToMethods()
            } label: {
                Text("Back")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .accessibilityIdentifier("cleanup-delete-back")

            cleanupCancelButton
        }
    }

    var cleanupCancelButton: some View {
        Button(role: .cancel) {
            cancelCleanup()
        } label: {
            Text("Cancel")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.bordered)
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
