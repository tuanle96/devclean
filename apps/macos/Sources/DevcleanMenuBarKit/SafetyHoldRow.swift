import AppKit
import SwiftUI

/// A single safety-hold row with keyboard, VoiceOver, and context-menu actions.
struct SafetyHoldRow: View {
    let entry: QuarantineEntry
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
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(entry.category.title).font(.subheadline)
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
                    Text("·").font(.caption2).foregroundStyle(.tertiary)
                    Text(UIFormatting.expiryText(entry.expiresAtUnix))
                        .font(.caption2.weight(.medium))
                        .foregroundStyle(UIFormatting.expiryColor(entry.expiresAtUnix))
                        .fixedSize()
                    Spacer(minLength: 4)
                    Button(action: onRestore) { Image(systemName: "arrow.uturn.backward") }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(isBusy)
                        .help("Restore to original path")
                        .accessibilityLabel("Restore \(entry.category.title)")
                        .accessibilityIdentifier("hold-restore-\(entry.id)")
                    Button(role: .destructive, action: onDelete) { Image(systemName: "trash") }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(isBusy)
                        .help("Permanently delete now")
                        .accessibilityLabel("Permanently delete \(entry.category.title)")
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
        .accessibilityLabel(
            "\(entry.category.title), \(ByteFormatting.string(entry.bytes)), safety hold, \(UIFormatting.expiryText(entry.expiresAtUnix))"
        )
        .accessibilityHint(entry.originalPath)
    }
}
