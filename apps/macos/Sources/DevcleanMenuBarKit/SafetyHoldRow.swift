import AppKit
import SwiftUI

/// A single safety-hold row with keyboard, VoiceOver, and context-menu actions.
struct SafetyHoldRow: View {
    let entry: QuarantineEntry
    /// Owning project resolved by the caller; nil for global caches, where the
    /// category itself is the clearest title.
    let projectName: String?
    let isBusy: Bool
    let onRestore: () -> Void
    let onDelete: () -> Void
    let onReveal: () -> Void
    let onCopyPath: () -> Void
    @State private var isHovering = false

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: entry.category.systemImage)
                .frame(width: 18)
                .foregroundStyle(.teal)
                .accessibilityHidden(true)
                .help(entry.category.title)
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(titleText)
                        .font(.subheadline.weight(.medium))
                        .lineLimit(1)
                    Spacer()
                    Text(ByteFormatting.string(entry.bytes))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                HStack(spacing: 6) {
                    Text((entry.originalPath as NSString).abbreviatingWithTildeInPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .help(entry.originalPath)
                    MetaChip(
                        text: UIFormatting.expiryText(entry.expiresAtUnix),
                        tint: UIFormatting.expiryColor(entry.expiresAtUnix)
                    )
                    Spacer(minLength: 4)
                    Button(action: onRestore) { Image(systemName: "arrow.uturn.backward") }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(isBusy)
                        .help("Restore to original path")
                        .accessibilityLabel("Restore \(titleText)")
                        .accessibilityIdentifier("hold-restore-\(entry.id)")
                    Button(role: .destructive, action: onDelete) { Image(systemName: "trash") }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(isBusy)
                        .help("Permanently delete now")
                        .accessibilityLabel("Permanently delete \(titleText)")
                        .accessibilityIdentifier("hold-delete-\(entry.id)")
                }
                .opacity(isHovering ? 1 : 0.55)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onHover { isHovering = $0 }
        .contextMenu {
            Button("Restore to Original Path", action: onRestore)
            Button("Reveal in Finder", action: onReveal)
            Button("Copy Path", action: onCopyPath)
            Divider()
            Button("Permanently Delete Now…", role: .destructive, action: onDelete)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(entry.originalPath)
    }

    private var titleText: String {
        projectName ?? entry.category.title
    }

    private var accessibilityLabel: String {
        var parts = [titleText]
        if titleText != entry.category.title {
            parts.append(entry.category.title)
        }
        parts.append(ByteFormatting.string(entry.bytes))
        parts.append("safety hold")
        parts.append(UIFormatting.expiryText(entry.expiresAtUnix))
        return parts.joined(separator: ", ")
    }
}
