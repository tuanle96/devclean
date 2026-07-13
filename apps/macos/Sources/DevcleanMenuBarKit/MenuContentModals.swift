import AppKit
import SwiftUI

extension MenuContentView {

    /// A dimming layer behind modal cards. Adapts to appearance (a hard black
    /// scrim is too heavy in Light Mode) and treats a tap as cancel, which is the
    /// standard expectation for dismissing a modal by clicking outside it.
    func dimScrim(onCancel: @escaping () -> Void) -> some View {
        Color.black
            .opacity(colorScheme == .dark ? 0.45 : 0.22)
            .contentShape(Rectangle())
            .onTapGesture(perform: onCancel)
            .accessibilityHidden(true)
    }

    func chooseInitialSectionIfNeeded() {
        guard !didChooseInitialSection, !model.isBusy, model.report != nil else { return }
        selectedSection = MenuSection.initial(
            safeCount: model.report?.candidates.count ?? 0,
            reviewCount: model.visibleReviewCandidates.count,
            holdCount: model.quarantineEntries.count
        )
        didChooseInitialSection = true
    }

    var cleanupConfirmationOverlay: some View {
        ZStack {
            dimScrim { cancelCleanup() }

            VStack(alignment: .leading, spacing: 14) {
                cleanupConfirmationContent
            }
            .padding(20)
            .frame(width: 380)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
            .overlay {
                RoundedRectangle(cornerRadius: 18)
                    .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
            .padding(20)
            .accessibilityAddTraits(.isModal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .zIndex(1)
        .accessibilityElement(children: .contain)
    }

    func holdPurgeConfirmationOverlay(_ request: HoldPurgeRequest) -> some View {
        ZStack {
            dimScrim { holdPurgeRequest = nil }

            VStack(alignment: .leading, spacing: 14) {
                Text(holdPurgeTitle(for: request))
                    .font(.headline)
                    .accessibilityFocused($overlayFocus, equals: .holdPurge)
                    .onAppear { overlayFocus = .holdPurge }

                Text(holdPurgeMessage(for: request))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                HStack(spacing: 8) {
                    Spacer()

                    Button("Cancel", role: .cancel) {
                        holdPurgeRequest = nil
                    }
                    .keyboardShortcut(.cancelAction)
                    .accessibilityIdentifier("hold-purge-cancel")

                    // Visually primary because deletion is this dialog's only
                    // purpose; Return deliberately stays unbound.
                    Button(role: .destructive) {
                        confirmHoldPurge(request)
                    } label: {
                        Text(holdPurgeButtonTitle(for: request))
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                    .disabled(model.isBusy)
                    .accessibilityIdentifier("hold-purge-confirm")
                }
            }
            .padding(20)
            .frame(width: 380)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18))
            .overlay {
                RoundedRectangle(cornerRadius: 18)
                    .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.35), radius: 18, y: 8)
            .padding(20)
            .accessibilityAddTraits(.isModal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .zIndex(1)
        .accessibilityElement(children: .contain)
    }

    func holdPurgeTitle(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one:
            return "Permanently delete this hold?"
        case .all:
            return "Delete all safety holds?"
        }
    }

    func holdPurgeMessage(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one(let entry):
            return
                "This releases \(ByteFormatting.string(entry.bytes)) immediately and cannot be undone. The original path will no longer be restorable."
        case .all:
            return
                "This permanently deletes all \(model.quarantineEntries.count) restorable items and releases \(ByteFormatting.string(model.safetyHoldBytes)). This cannot be undone."
        }
    }

    func holdPurgeButtonTitle(for request: HoldPurgeRequest) -> String {
        switch request {
        case .one(let entry):
            return "Delete \(ByteFormatting.string(entry.bytes)) Now"
        case .all:
            return "Delete All · \(ByteFormatting.string(model.safetyHoldBytes))"
        }
    }

    func confirmHoldPurge(_ request: HoldPurgeRequest) {
        holdPurgeRequest = nil
        switch request {
        case .one(let entry):
            model.purgeSafetyHold(entry)
        case .all:
            model.purgeAllSafetyHolds()
        }
    }

}
