import AppKit
import SwiftUI

public struct MenuContentView: View {
    @ObservedObject private var model: AppModel
    @State private var showingConfirmation = false

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
          VStack(alignment: .leading, spacing: 12) {
              header
              learningOverview
              Divider()
              content
              safetyHoldSummary
              messages
            Divider()
            actions
            footer
        }
        .padding(16)
        .frame(width: 380)
        .confirmationDialog(
              model.usesSafetyHold ? "Hold selected artifacts?" : "Clean selected artifacts?",
            isPresented: $showingConfirmation,
            titleVisibility: .visible
        ) {
            Button(
                  model.usesSafetyHold
                      ? "Hold \(ByteFormatting.string(model.selectedBytes))"
                      : "Clean \(ByteFormatting.string(model.selectedBytes))",
                role: .destructive
            ) {
                model.cleanSelected()
            }
            Button("Cancel", role: .cancel) {}
          } message: {
              Text(
                  model.usesSafetyHold
                      ? "Rust validates every path again, then moves it into a restorable safety hold. Disk space is released only after purge."
                      : "Rust validates every selected path again before atomic quarantine and deletion."
              )
          }
          .task { model.initialLoad() }
      }

      private var learningOverview: some View {
          HStack(spacing: 16) {
              learningMetric(
                  value: "\(model.learningSummary.observedDays)d",
                  label: "observed"
              )
              learningMetric(
                  value: growthText,
                  label: "growth"
              )
              learningMetric(
                  value: "\(model.visibleReviewCandidates.count)",
                  label: "review"
              )
              Spacer()
              Image(systemName: "brain.head.profile")
                  .foregroundStyle(.secondary)
                  .accessibilityLabel("Learning Mode")
          }
          .padding(10)
          .background(.quaternary.opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
      }

      private func learningMetric(value: String, label: String) -> some View {
          VStack(alignment: .leading, spacing: 1) {
              Text(value)
                  .font(.caption.weight(.semibold).monospacedDigit())
              Text(label)
                  .font(.caption2)
                  .foregroundStyle(.secondary)
          }
      }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 3) {
                Text("Devclean")
                    .font(.headline)
                Text(freeSpaceText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 3) {
                Text(ByteFormatting.string(model.report?.totalBytes ?? 0))
                    .font(.headline.monospacedDigit())
                Text("reclaimable")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    @ViewBuilder
    private var content: some View {
        if model.phase == .scanning, model.report == nil {
            HStack(spacing: 10) {
                ProgressView()
                Text("Scanning development artifacts…")
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, minHeight: 120)
          } else if let report = model.report,
                    !report.candidates.isEmpty || !model.visibleReviewCandidates.isEmpty
          {
              ScrollView {
                  LazyVStack(alignment: .leading, spacing: 0) {
                      if !report.candidates.isEmpty {
                          HStack {
                              Text("Safe candidates")
                                  .font(.subheadline.weight(.semibold))
                              Spacer()
                              Button("All") { model.selectAll() }
                                  .buttonStyle(.plain)
                              Text("·").foregroundStyle(.tertiary)
                              Button("None") { model.selectNone() }
                                  .buttonStyle(.plain)
                          }
                          .padding(.bottom, 4)
                      }
                      ForEach(report.candidates) { candidate in
                          candidateRow(candidate)
                          if candidate.id != report.candidates.last?.id {
                              Divider().padding(.leading, 30)
                          }
                      }
                      if !model.visibleReviewCandidates.isEmpty {
                            Text("Learning Mode · approvals")
                              .font(.subheadline.weight(.semibold))
                              .padding(.top, report.candidates.isEmpty ? 0 : 12)
                              .padding(.bottom, 4)
                          ForEach(model.visibleReviewCandidates) { candidate in
                              reviewCandidateRow(candidate)
                              if candidate.id != model.visibleReviewCandidates.last?.id {
                                  Divider().padding(.leading, 30)
                              }
                          }
                      }
                  }
              }
              .frame(height: candidateListHeight)
        } else {
            VStack(spacing: 8) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.green)
                    .accessibilityHidden(true)
                Text("Disk looks clean")
                    .font(.headline)
                Text("No rebuildable artifacts match the current filters.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity, minHeight: 140)
        }
    }

    private func candidateRow(_ candidate: CleanupCandidate) -> some View {
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
                    HStack {
                        Text(candidate.category.title)
                            .font(.subheadline)
                        Spacer()
                        Text(ByteFormatting.string(candidate.bytes))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                    Text(candidate.path)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .help(candidate.path)
                }
            }
            .frame(minHeight: 44)
        }
          .toggleStyle(.checkbox)
            .contextMenu {
                if let rule = candidate.approvedRule {
                    Button("Revoke learned \(rule.title) approval") {
                        model.revokeReviewApproval(path: candidate.path, rule: rule)
                    }
                }
                Button("Always select this safe artifact") {
                  model.recordFeedback(.alwaysClean, path: candidate.path)
              }
              Button("Never clean this path") {
                  model.recordFeedback(.neverClean, path: candidate.path)
              }
          }
        .accessibilityLabel("\(candidate.category.title), \(ByteFormatting.string(candidate.bytes))")
        .accessibilityHint(candidate.path)
      }

      private func reviewCandidateRow(_ candidate: ReviewCandidate) -> some View {
          HStack(spacing: 10) {
              Image(systemName: "magnifyingglass")
                  .frame(width: 18)
                  .foregroundStyle(.orange)
                  .accessibilityHidden(true)
              VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(reviewTitle(candidate))
                            .font(.subheadline)
                        Spacer()
                        Text(ByteFormatting.string(candidate.bytes))
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                        if candidate.suggestedRule != nil {
                            Button(candidate.approved ? "Revoke" : "Approve") {
                                if candidate.approved {
                                    model.revokeReviewApproval(
                                        path: candidate.path,
                                        rule: candidate.suggestedRule
                                    )
                                } else {
                                    model.approveReviewCandidate(candidate)
                                }
                            }
                            .buttonStyle(.borderless)
                            .controlSize(.mini)
                        }
                  }
                  Text(candidate.path)
                      .font(.caption)
                      .foregroundStyle(.secondary)
                      .lineLimit(1)
                      .truncationMode(.middle)
                      .help("\(candidate.path)\n\(candidate.reason)")
              }
          }
          .frame(minHeight: 44)
            .contextMenu {
                if candidate.suggestedRule != nil, !candidate.approved {
                    Button("Approve this scanner-owned rule") {
                        model.approveReviewCandidate(candidate)
                    }
                }
                if candidate.approved {
                    Button("Revoke learned approval") {
                        model.revokeReviewApproval(
                            path: candidate.path,
                            rule: candidate.suggestedRule
                        )
                    }
                }
                Button("Ignore this path") {
                  model.recordFeedback(.neverClean, path: candidate.path)
              }
          }
          .accessibilityLabel("Review only, \(ByteFormatting.string(candidate.bytes))")
            .accessibilityHint(candidate.reason)
        }

        private func reviewTitle(_ candidate: ReviewCandidate) -> String {
            if candidate.approved {
                return "Approved · waiting for cleanup threshold"
            }
            return candidate.suggestedRule?.title ?? "Needs review"
        }

      @ViewBuilder
      private var safetyHoldSummary: some View {
          if !model.quarantineEntries.isEmpty {
              Label(
                  "\(model.quarantineEntries.count) safety holds retain \(ByteFormatting.string(model.safetyHoldBytes)) until purge",
                  systemImage: "clock.arrow.circlepath"
              )
              .font(.caption)
              .foregroundStyle(.orange)
          }
      }

    @ViewBuilder
    private var messages: some View {
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

    private var actions: some View {
        HStack(spacing: 8) {
            Button {
                model.scan()
            } label: {
                Label("Scan", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r")
            .disabled(model.isBusy)

            Spacer()

            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
            }
            Button {
                showingConfirmation = true
            } label: {
                  Label(
                      model.usesSafetyHold ? "Hold Selected" : "Clean Selected",
                      systemImage: model.usesSafetyHold ? "archivebox" : "trash"
                  )
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
            .disabled(model.isBusy || model.selectedPaths.isEmpty)
        }
        .controlSize(.large)
    }

    private var footer: some View {
        HStack {
            if #available(macOS 14.0, *) {
                SettingsLink {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            } else {
                Button {
                    NSApp.sendAction(Selector(("showSettingsWindow:")), to: nil, from: nil)
                } label: {
                    Label("Settings…", systemImage: "gear")
                }
                .buttonStyle(.plain)
            }
            Spacer()
            Button("Quit") { NSApp.terminate(nil) }
                .buttonStyle(.plain)
                .keyboardShortcut("q")
        }
        .font(.caption)
    }

      private var freeSpaceText: String {
        guard let bytes = model.availableBytes else { return "Free space unavailable" }
        return "\(ByteFormatting.string(bytes)) free"
      }

      private var growthText: String {
          let growth = model.learningSummary.growthBytes
          let prefix = growth < 0 ? "−" : "+"
          return prefix + ByteFormatting.string(growth.magnitude)
      }

      private var candidateListHeight: CGFloat {
          let rowCount = (model.report?.candidates.count ?? 0)
              + model.visibleReviewCandidates.count
          let sectionCount = (model.report?.candidates.isEmpty == false ? 1 : 0)
              + (model.visibleReviewCandidates.isEmpty ? 0 : 1)
          return min(300, max(90, CGFloat(rowCount * 46 + sectionCount * 30)))
      }
  }

  public struct SettingsView: View {
      @ObservedObject private var model: AppModel
    @AppStorage(PreferenceKeys.roots) private var roots = ""
    @AppStorage(PreferenceKeys.olderThan) private var olderThan = "7d"
    @AppStorage(PreferenceKeys.minimumSize) private var minimumSize = "100MiB"
    @AppStorage(PreferenceKeys.buildOutputs) private var buildOutputs = false
    @AppStorage(PreferenceKeys.testCaches) private var testCaches = false
    @AppStorage(PreferenceKeys.globalCaches) private var globalCaches = false
      @AppStorage(PreferenceKeys.expensiveCaches) private var expensiveCaches = false
      @AppStorage(PreferenceKeys.learningMode) private var learningMode = true
      @AppStorage(PreferenceKeys.safetyHoldDays) private var safetyHoldDays = 7
      @AppStorage(PreferenceKeys.anonymousDiagnostics) private var anonymousDiagnostics = false

      public init(model: AppModel) {
          self.model = model
      }

    public var body: some View {
        Form {
            Section("Scan roots") {
                TextEditor(text: $roots)
                    .font(.body.monospaced())
                    .frame(minHeight: 88)
                    .overlay(alignment: .topLeading) {
                        if roots.isEmpty {
                            Text("Leave empty for ~/Dev, ~/Projects and ~/Documents/Codex")
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 5)
                                .padding(.vertical, 8)
                                .allowsHitTesting(false)
                        }
                    }
                Text("Enter one directory per line. Tilde paths are supported.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Filters") {
                TextField("Older than", text: $olderThan, prompt: Text("7d"))
                TextField("Minimum size", text: $minimumSize, prompt: Text("100MiB"))
                Text("Leave either field empty to disable that filter.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

              Section("Additional categories") {
                Toggle("Build outputs", isOn: $buildOutputs)
                Toggle("Test caches", isOn: $testCaches)
                Toggle("Package and tool caches", isOn: $globalCaches)
                Toggle("Runtime and model caches", isOn: $expensiveCaches)
                Text("Runtime and model caches can be expensive to download again.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
              }

              Section("Learning Mode") {
                  Toggle("Observe artifact growth locally", isOn: $learningMode)
                  Stepper(
                      "Safety hold: \(safetyHoldDays == 0 ? "off" : "\(safetyHoldDays) days")",
                      value: $safetyHoldDays,
                      in: 0...30
                  )
                  Text("A safety hold is restorable but still uses disk space until it is purged. Review-only observations are never cleanable automatically.")
                      .font(.caption)
                      .foregroundStyle(.secondary)
                  Button("Reset local learning history") {
                      model.resetLearningData()
                  }
              }

              Section("Safety holds") {
                  if model.quarantineEntries.isEmpty {
                      Text("No restorable holds.")
                          .foregroundStyle(.secondary)
                  } else {
                      ForEach(model.quarantineEntries.prefix(5)) { entry in
                          HStack {
                              VStack(alignment: .leading) {
                                  Text(entry.originalPath)
                                      .lineLimit(1)
                                      .truncationMode(.middle)
                                  Text(ByteFormatting.string(entry.bytes))
                                      .font(.caption)
                                      .foregroundStyle(.secondary)
                              }
                              Spacer()
                              Button("Restore") {
                                  model.restoreSafetyHold(entry)
                              }
                              .disabled(model.isBusy)
                          }
                      }
                  }
                  Button("Purge expired holds") {
                      model.purgeExpiredSafetyHolds()
                  }
                  .disabled(model.isBusy)
              }

              Section("Diagnostics") {
                  Toggle(
                      "Share anonymous errors with Sentry",
                      isOn: Binding(
                          get: { anonymousDiagnostics },
                          set: { value in
                              anonymousDiagnostics = value
                              model.setRemoteDiagnosticsConsent(value)
                          }
                      )
                  )
                  .disabled(!model.isRemoteMonitoringConfigured)
                  Text(
                      model.isRemoteMonitoringConfigured
                          ? "Opt-in. Remote events contain error fingerprints and aggregate buckets only—never paths, usernames, or project names."
                          : "Sentry provider is built in but no DSN is configured. Local structured logs remain active."
                  )
                  .font(.caption)
                  .foregroundStyle(.secondary)
                  Button("Open local logs") {
                      model.openLocalLogs()
                  }
              }

            Section("Safety") {
                Label("Git-tracked files remain protected", systemImage: "lock.shield")
                Label("Docker volumes are never touched", systemImage: "externaldrive.badge.xmark")
                Label("Every clean performs a fresh Rust safety scan", systemImage: "checkmark.shield")
            }
        }
        .formStyle(.grouped)
          .frame(width: 560, height: 760)
      }
  }
