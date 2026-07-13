import AppKit
import SwiftUI

extension MenuContentView {

    var sectionPicker: some View {
        Picker("Task", selection: $selectedSection) {
            Text(sectionLabel("Clean", count: model.report?.candidates.count ?? 0))
                .tag(MenuSection.clean)
            Text(sectionLabel("Review", count: model.visibleReviewCandidates.count))
                .tag(MenuSection.review)
            Text(sectionLabel("Holds", count: model.quarantineEntries.count))
                .tag(MenuSection.holds)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .accessibilityIdentifier("task-picker")
    }

    func sectionLabel(_ title: String, count: Int) -> String {
        count > 0 ? "\(title) \(count)" : title
    }

    @ViewBuilder
    var aiRecommendationStatus: some View {
        if aiInsightsEnabled, aiMonitoringEnabled {
            switch aiInsights.state {
            case .generating:
                HStack(spacing: 8) {
                    ProgressView()
                        .controlSize(.small)
                    Text("AI is reviewing the latest scan…")
                        .font(.caption)
                    Spacer()
                }
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("ai-monitoring-progress")
            case .result(let insight):
                Button {
                    showingAIInsight = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "sparkles")
                            .foregroundStyle(.purple)
                        Text("\(insight.recommendations.count) AI recommendations ready")
                            .font(.caption.weight(.semibold))
                        Spacer()
                        Text("View")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(9)
                    .background(.purple.opacity(0.1), in: RoundedRectangle(cornerRadius: 9))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("ai-monitoring-result")
            case .idle, .unavailable, .failed:
                EmptyView()
            }
        }
    }

    @ViewBuilder
    var sectionSummary: some View {
        switch selectedSection {
        case .clean:
            if let report = model.report, !report.candidates.isEmpty {
                HStack {
                    Label(
                        "\(report.candidates.count) safe artifacts",
                        systemImage: "checkmark.shield"
                    )
                    Spacer()
                    if aiInsightsEnabled {
                        Button {
                            presentAIInsight()
                        } label: {
                            Label("Recommend", systemImage: "sparkles")
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(model.isBusy)
                        .accessibilityIdentifier("ai-recommend-open-clean")
                    }
                    Button("All") { model.selectAll() }
                        .buttonStyle(.plain)
                        .disabled(model.isBusy)
                    Text("·").foregroundStyle(.tertiary)
                    Button("None") { model.selectNone() }
                        .buttonStyle(.plain)
                        .disabled(model.isBusy)
                }
                .font(.subheadline)
            }
        case .review:
            HStack(spacing: 6) {
                Image(systemName: "checkmark.bubble")
                    .foregroundStyle(.orange)
                    .accessibilityHidden(true)
                Text("Observation & Approvals")
                    .font(.subheadline.weight(.semibold))
                Spacer()
                Text(observedForText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if aiInsightsEnabled, !model.visibleReviewCandidates.isEmpty {
                    Button {
                        presentAIInsight()
                    } label: {
                        Label("Recommend", systemImage: "sparkles")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(model.isBusy)
                    .accessibilityIdentifier("ai-recommend-open-review")
                }
            }
        case .holds:
            HStack(spacing: 10) {
                Image(systemName: "clock.arrow.circlepath")
                    .font(.title3)
                    .foregroundStyle(.teal)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(ByteFormatting.string(model.safetyHoldBytes)) held safely")
                        .font(.subheadline.weight(.semibold))
                    Text("Restore items, or permanently delete them to reclaim space now.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
        }
    }

    var observedForText: String {
        let days = model.learningSummary.observedDays
        if days <= 0 { return "Not yet observed" }
        return "Observing for \(days) \(days == 1 ? "day" : "days")"
    }

    var header: some View {
        VStack(alignment: .leading, spacing: 9) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("DevCleaner")
                        .font(.headline)
                    Text(subtitleText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 2) {
                    Text(headlineValue)
                        .font(.title3.weight(.semibold).monospacedDigit())
                        .foregroundStyle(headlineColor)
                        .contentTransition(.numericText())
                    Text(headlineLabel)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
            }
            capacityBar
        }
    }

    @ViewBuilder
    var capacityBar: some View {
        if let total = model.volumeTotalBytes, total > 0,
            let segments = CapacityBar.segments(
                total: total,
                free: model.volumeFreeBytes,
                held: model.safetyHoldBytes,
                reclaimable: model.report?.totalBytes ?? 0
            )
        {
            VStack(alignment: .leading, spacing: 5) {
                GeometryReader { geometry in
                    HStack(spacing: 1) {
                        ForEach(segments) { segment in
                            segment.color
                                .frame(
                                    width: max(
                                        2,
                                        geometry.size.width * CGFloat(Double(segment.bytes) / Double(total))
                                    )
                                )
                        }
                    }
                }
                .frame(height: 6)
                .clipShape(Capsule())
                .accessibilityHidden(true)

                HStack(spacing: 12) {
                    ForEach(segments.filter { $0.id != "other" }) { segment in
                        HStack(spacing: 4) {
                            Circle()
                                .fill(segment.color)
                                .frame(width: 7, height: 7)
                            Text("\(ByteFormatting.string(segment.bytes)) \(segment.label)")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    var subtitleText: String {
        if model.phase == .scanning {
            return "Scanning…"
        }
        if let date = model.lastScanDate {
            return "Last scan \(UIFormatting.relativePast(date))"
        }
        // Free space is shown by the capacity bar below; keep the subtitle about
        // scan freshness so two different "free" figures never appear together.
        return "Not scanned yet"
    }

    var headlineValue: String {
        let reclaimable = model.report?.totalBytes ?? 0
        if reclaimable > 0 {
            return ByteFormatting.string(reclaimable)
        }
        if model.safetyHoldBytes > 0 {
            return "All clean"
        }
        return "Up to date"
    }

    var headlineLabel: String {
        if (model.report?.totalBytes ?? 0) > 0 {
            return "ready to clean"
        }
        if model.safetyHoldBytes > 0 {
            return "\(ByteFormatting.string(model.safetyHoldBytes)) held safely"
        }
        return "nothing to reclaim"
    }

    var headlineColor: Color {
        (model.report?.totalBytes ?? 0) > 0 ? .accentColor : .green
    }

    @ViewBuilder
    var content: some View {
        if model.phase == .scanning, model.report == nil {
            HStack(spacing: 10) {
                ProgressView()
                Text("Scanning development artifacts…")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 120)
        } else {
            switch selectedSection {
            case .clean:
                cleanContent
            case .review:
                reviewContent
            case .holds:
                holdsContent
            }
        }
    }

    @ViewBuilder
    var cleanContent: some View {
        if let candidates = model.report?.candidates, !candidates.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(candidates) { candidate in
                        CandidateRow(candidate: candidate, model: model)
                        if candidate.id != candidates.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: candidates.count, rowHeight: 48))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("No safe artifacts to clean")
                    .font(.headline)
                Text("Scan again, adjust filters in Settings, or manage space already in safety holds.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 300)
                HStack(spacing: 8) {
                    Button("Open Settings…") { openSettingsWindow() }
                        .buttonStyle(.bordered)
                    if !model.quarantineEntries.isEmpty {
                        Button("View \(ByteFormatting.string(model.safetyHoldBytes)) in Holds") {
                            selectedSection = .holds
                        }
                        .buttonStyle(.bordered)
                        .accessibilityIdentifier("view-holds")
                    }
                }
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    @ViewBuilder
    var reviewContent: some View {
        if !model.visibleReviewCandidates.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(model.visibleReviewCandidates) { candidate in
                        ReviewCandidateRow(candidate: candidate, model: model)
                        if candidate.id != model.visibleReviewCandidates.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: model.visibleReviewCandidates.count, rowHeight: 52))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "checkmark.seal")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("Nothing needs review")
                    .font(.headline)
                Text("Items DevCleaner isn't sure about wait here. Approve one to allow cleaning it in future scans.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 310)
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    @ViewBuilder
    var holdsContent: some View {
        if !model.quarantineEntries.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(model.quarantineEntries) { entry in
                        safetyHoldRow(entry)
                        if entry.id != model.quarantineEntries.last?.id {
                            Divider().padding(.leading, 30)
                        }
                    }
                }
            }
            .frame(height: listHeight(for: model.quarantineEntries.count, rowHeight: 58))
        } else {
            VStack(spacing: 9) {
                Image(systemName: "archivebox")
                    .font(.system(size: 28))
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)
                Text("No safety holds")
                    .font(.headline)
                Text(
                    "Artifacts cleaned with Safety Hold will remain restorable here until their retention period expires."
                )
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 310)
            }
            .frame(maxWidth: .infinity, minHeight: 150)
        }
    }

    func safetyHoldRow(_ entry: QuarantineEntry) -> some View {
        SafetyHoldRow(
            entry: entry,
            isBusy: model.isBusy,
            onRestore: { model.restoreSafetyHold(entry) },
            onDelete: { holdPurgeRequest = .one(entry) },
            onReveal: { FinderActions.revealInFinder(entry.originalPath) },
            onCopyPath: { FinderActions.copyPath(entry.originalPath) }
        )
    }

    @ViewBuilder
    var messages: some View {
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
}
