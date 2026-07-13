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
                    // AI identity lives in the sparkles glyph; the surface stays a
                    // neutral material so the banner doesn't fight the accent color.
                    .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 9))
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
                    Button("All") { model.selectAll() }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(model.isBusy)
                        .accessibilityLabel("Select all artifacts")
                    Button("None") { model.selectNone() }
                        .buttonStyle(.borderless)
                        .controlSize(.small)
                        .disabled(model.isBusy)
                        .accessibilityLabel("Deselect all artifacts")
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
                    Text(holdsSummaryCaption)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer(minLength: 4)
                if !model.quarantineEntries.isEmpty {
                    // Destructive bulk action lives in an overflow menu near the top
                    // of the window, away from the hasty-click zone at the bottom.
                    Menu {
                        Button(role: .destructive) {
                            holdPurgeRequest = .all
                        } label: {
                            Label(
                                "Delete All · \(ByteFormatting.string(model.safetyHoldBytes))…",
                                systemImage: "trash"
                            )
                        }
                        .accessibilityIdentifier("hold-delete-all")
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                    .menuStyle(.button)
                    .buttonStyle(.borderless)
                    .fixedSize()
                    .disabled(model.isBusy)
                    .accessibilityLabel("Safety hold actions")
                }
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
        }
    }

    var holdsSummaryCaption: String {
        // min(expiresAtUnix) is the soonest-expiring hold — the retention fact
        // that matters — which is not necessarily the oldest one.
        guard let soonest = model.quarantineEntries.map(\.expiresAtUnix).min() else {
            return "Restore items, or permanently delete them to reclaim space now."
        }
        let phrase = UIFormatting.expiryText(soonest)
        return "Next hold \(phrase.prefix(1).lowercased() + phrase.dropFirst())."
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
                // The legend below only names the three actionable slices; the bar
                // itself explains the remaining gray bulk and reads as one element.
                .help(capacityBarHelp(segments))
                .accessibilityElement()
                .accessibilityLabel(capacityBarSummary(segments))

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
                .accessibilityHidden(true)
            }
        }
    }

    func capacityBarHelp(_ segments: [CapacitySegment]) -> String {
        guard let other = segments.first(where: { $0.id == "other" }) else {
            return "Disk capacity"
        }
        return "The gray area is \(ByteFormatting.string(other.bytes)) used by everything else on this disk."
    }

    func capacityBarSummary(_ segments: [CapacitySegment]) -> String {
        "Disk capacity: "
            + segments
            .map { "\(ByteFormatting.string($0.bytes)) \($0.label)" }
            .joined(separator: ", ")
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
            // List (not ScrollView) so rows get selection, arrow-key navigation,
            // and standard separators for free.
            List(selection: $listFocus) {
                ForEach(candidates) { candidate in
                    CandidateRow(candidate: candidate, model: model)
                        .listRowInsets(EdgeInsets(top: 2, leading: 4, bottom: 2, trailing: 4))
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
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
                    openSettingsButton {
                        Text("Open Settings…")
                    }
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
            List(selection: $listFocus) {
                ForEach(model.visibleReviewCandidates) { candidate in
                    ReviewCandidateRow(candidate: candidate, model: model)
                        .listRowInsets(EdgeInsets(top: 2, leading: 4, bottom: 2, trailing: 4))
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
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
            List(selection: $listFocus) {
                ForEach(model.quarantineEntries) { entry in
                    safetyHoldRow(entry)
                        .listRowInsets(EdgeInsets(top: 2, leading: 4, bottom: 2, trailing: 4))
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
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
            projectName: holdProjectName(entry),
            isBusy: model.isBusy,
            onRestore: { model.restoreSafetyHold(entry) },
            onDelete: { holdPurgeRequest = .one(entry) },
            onReveal: { FinderActions.revealInFinder(entry.originalPath) },
            onCopyPath: { FinderActions.copyPath(entry.originalPath) }
        )
    }

    /// Global caches keep their category as the title; everything else is named
    /// by its owning project.
    func holdProjectName(_ entry: QuarantineEntry) -> String? {
        switch entry.category {
        case .globalCache, .expensiveGlobalCache:
            nil
        default:
            UIFormatting.projectName(
                forPath: entry.originalPath,
                workspaceRoots: model.workspaceRoots
            )
        }
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
