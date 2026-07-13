import AppKit
import SwiftUI

struct CandidateRow: View {
    let candidate: CleanupCandidate
    @ObservedObject var model: AppModel

    var body: some View {
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
                    HStack(spacing: 6) {
                        Text(candidate.category.title).font(.subheadline)
                        Spacer()
                        if let age = UIFormatting.ageText(candidate.modifiedAtUnix) {
                            Text(age).font(.caption2).foregroundStyle(.tertiary)
                        }
                        Text(ByteFormatting.string(candidate.bytes))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    Text((candidate.path as NSString).abbreviatingWithTildeInPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .help(candidate.path)
                }
            }
            .padding(.vertical, 4)
        }
        .toggleStyle(.checkbox)
        .disabled(model.isBusy)
        .contextMenu {
            Button("Reveal in Finder") { FinderActions.revealInFinder(candidate.path) }
            Button("Copy Path") { FinderActions.copyPath(candidate.path) }
            Divider()
            if let rule = candidate.approvedRule {
                Button("Revoke Learned \(rule.title) Approval") {
                    model.revokeReviewApproval(path: candidate.path, rule: rule)
                }
            }
            Button("Always Select This Artifact") {
                model.recordFeedback(.alwaysClean, path: candidate.path)
            }
            Button("Never Clean This Path") {
                model.recordFeedback(.neverClean, path: candidate.path)
            }
        }
        .accessibilityLabel(
            "\(candidate.category.title), \(ByteFormatting.string(candidate.bytes)), rebuildable"
        )
        .accessibilityHint(candidate.path)
    }
}

struct ReviewCandidateRow: View {
    let candidate: ReviewCandidate
    @ObservedObject var model: AppModel

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .frame(width: 18)
                .foregroundStyle(.orange)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(title).font(.subheadline)
                    Spacer()
                    if let age = UIFormatting.ageText(candidate.modifiedAtUnix) {
                        Text(age).font(.caption2).foregroundStyle(.tertiary)
                    }
                    Text(ByteFormatting.string(candidate.bytes))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                    if candidate.suggestedRule != nil {
                        Button(candidate.approved ? "Revoke" : "Approve", action: toggleApproval)
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .accessibilityLabel(
                                "\(candidate.approved ? "Revoke approval for" : "Approve cleaning") "
                                    + (candidate.suggestedRule?.title ?? "this artifact")
                            )
                    }
                }
                Text((candidate.path as NSString).abbreviatingWithTildeInPath)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help("\(candidate.path)\n\(candidate.reason)")
            }
        }
        .padding(.vertical, 4)
        .contextMenu {
            Button("Reveal in Finder") { FinderActions.revealInFinder(candidate.path) }
            Button("Copy Path") { FinderActions.copyPath(candidate.path) }
            Divider()
            if candidate.suggestedRule != nil, !candidate.approved {
                Button("Approve Cleaning \(candidate.suggestedRule?.title ?? "This") Here") {
                    model.approveReviewCandidate(candidate)
                }
            }
            if candidate.approved {
                Button("Revoke Learned Approval") {
                    model.revokeReviewApproval(path: candidate.path, rule: candidate.suggestedRule)
                }
            }
            Button("Ignore This Path") {
                model.recordFeedback(.neverClean, path: candidate.path)
            }
        }
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(candidate.reason)
    }

    private var title: String {
        candidate.approved
            ? "Approved · will appear in Clean when eligible"
            : candidate.suggestedRule?.title ?? "Needs review"
    }

    private var accessibilityLabel: String {
        let type = candidate.suggestedRule?.title ?? "Review artifact"
        let state = candidate.approved ? "approved" : "needs review"
        return "\(type), \(ByteFormatting.string(candidate.bytes)), \(state)"
    }

    private func toggleApproval() {
        if candidate.approved {
            model.revokeReviewApproval(path: candidate.path, rule: candidate.suggestedRule)
        } else {
            model.approveReviewCandidate(candidate)
        }
    }
}
