//! Deterministic, local-first suggestions derived from scan and aggregate history data.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::history::HistorySummary;
use crate::model::{Category, ScanReport};
use crate::workspace::WorkspaceSummary;

/// Machine-readable analysis finding type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightKind {
    /// Rebuildable artifacts have not changed within the configured stale window.
    StaleArtifacts,
    /// A category grew across multiple aggregate scan snapshots.
    RepeatedGrowth,
    /// Previous cleanup attempts recorded failures.
    CleanupFailures,
    /// Multiple candidates are concentrated in one recognized workspace.
    WorkspaceConcentration,
    /// More history is needed before trend heuristics become meaningful.
    HistoryBaseline,
    /// The current scan found no reclaimable candidates.
    NoCandidates,
    /// No actionable anomaly was detected.
    Stable,
}

/// Relative importance of an analysis insight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightSeverity {
    Info,
    Opportunity,
    Warning,
}

/// One actionable, path-free suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisInsight {
    pub kind: InsightKind,
    pub severity: InsightSeverity,
    pub title: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_index: Option<usize>,
    pub candidate_count: usize,
    pub bytes: u64,
    pub occurrences: u64,
}

/// Local intelligent suggestions and the metrics used to derive them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub days: u64,
    pub stale_after_days: u64,
    pub candidate_count: usize,
    pub current_reclaimable_bytes: u64,
    pub reclaimable_change_bytes: i64,
    pub history_scan_count: u64,
    pub history_cleanup_count: u64,
    pub workspaces: Vec<WorkspaceSummary>,
    pub insights: Vec<AnalysisInsight>,
}

#[derive(Debug, Default)]
struct StaleAggregate {
    candidate_count: usize,
    bytes: u64,
}

/// Correlates a current read-only scan with path-free local history aggregates.
#[must_use]
pub fn analyze(
    report: &ScanReport,
    history: &HistorySummary,
    stale_after_days: u64,
) -> AnalysisReport {
    analyze_at(
        report,
        history,
        stale_after_days,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
    )
}

fn analyze_at(
    report: &ScanReport,
    history: &HistorySummary,
    stale_after_days: u64,
    now_unix: u64,
) -> AnalysisReport {
    let mut insights = stale_insights(report, stale_after_days, now_unix);
    insights.extend(growth_insights(history));
    if let Some(insight) = cleanup_failure_insight(history) {
        insights.push(insight);
    }
    insights.extend(workspace_insights(report));
    if let Some(insight) = fallback_insight(report, history, insights.is_empty()) {
        insights.push(insight);
    }

    AnalysisReport {
        days: history.days,
        stale_after_days,
        candidate_count: report.candidates.len(),
        current_reclaimable_bytes: report.total_bytes,
        reclaimable_change_bytes: history.reclaimable_change_bytes,
        history_scan_count: history.scan_count,
        history_cleanup_count: history.cleanup_count,
        workspaces: report.workspaces.clone(),
        insights,
    }
}

fn stale_insights(
    report: &ScanReport,
    stale_after_days: u64,
    now_unix: u64,
) -> Vec<AnalysisInsight> {
    let stale_cutoff = now_unix.saturating_sub(stale_after_days.saturating_mul(86_400));
    let mut stale = BTreeMap::<Category, StaleAggregate>::new();
    for candidate in &report.candidates {
        if candidate
            .modified_at_unix
            .is_some_and(|modified| modified <= stale_cutoff)
        {
            let aggregate = stale.entry(candidate.category).or_default();
            aggregate.candidate_count = aggregate.candidate_count.saturating_add(1);
            aggregate.bytes = aggregate.bytes.saturating_add(candidate.bytes);
        }
    }
    stale
        .into_iter()
        .map(|(category, aggregate)| AnalysisInsight {
            kind: InsightKind::StaleArtifacts,
            severity: InsightSeverity::Opportunity,
            title: format!("Stale {category} artifacts"),
            message: format!(
                "{} artifacts have not changed for at least {stale_after_days} days.",
                aggregate.candidate_count
            ),
            category: Some(category),
            workspace_index: None,
            candidate_count: aggregate.candidate_count,
            bytes: aggregate.bytes,
            occurrences: 0,
        })
        .collect()
}

fn growth_insights(history: &HistorySummary) -> Vec<AnalysisInsight> {
    history
        .category_growth_events
        .iter()
        .filter(|(_, occurrences)| **occurrences > 0)
        .map(|(category, occurrences)| AnalysisInsight {
            kind: InsightKind::RepeatedGrowth,
            severity: InsightSeverity::Opportunity,
            title: format!("Repeated {category} growth"),
            message: format!(
                "Aggregate {category} bytes increased on {occurrences} recorded scans in the {}-day window.",
                history.days
            ),
            category: Some(*category),
            workspace_index: None,
            candidate_count: 0,
            bytes: history
                .category_change_bytes
                .get(category)
                .copied()
                .unwrap_or(0)
                .max(0)
                .unsigned_abs(),
            occurrences: *occurrences,
        })
        .collect()
}

fn cleanup_failure_insight(history: &HistorySummary) -> Option<AnalysisInsight> {
    (history.failures > 0).then(|| AnalysisInsight {
        kind: InsightKind::CleanupFailures,
        severity: InsightSeverity::Warning,
        title: "Review cleanup failures".to_owned(),
        message: format!(
            "{} cleanup failures were recorded in the {}-day history window.",
            history.failures, history.days
        ),
        category: None,
        workspace_index: None,
        candidate_count: 0,
        bytes: 0,
        occurrences: history.failures,
    })
}

fn workspace_insights(report: &ScanReport) -> Vec<AnalysisInsight> {
    report
        .workspaces
        .iter()
        .enumerate()
        .filter(|(_, workspace)| workspace.candidate_count >= 2)
        .map(|(index, workspace)| AnalysisInsight {
            kind: InsightKind::WorkspaceConcentration,
            severity: InsightSeverity::Opportunity,
            title: format!("Workspace {} concentrates rebuildable artifacts", index + 1),
            message: format!(
                "{} candidates share one workspace root and can be considered together.",
                workspace.candidate_count
            ),
            category: None,
            workspace_index: Some(index + 1),
            candidate_count: workspace.candidate_count,
            bytes: workspace.total_bytes,
            occurrences: 0,
        })
        .collect()
}

fn fallback_insight(
    report: &ScanReport,
    history: &HistorySummary,
    otherwise_stable: bool,
) -> Option<AnalysisInsight> {
    if report.candidates.is_empty() {
        Some(AnalysisInsight {
            kind: InsightKind::NoCandidates,
            severity: InsightSeverity::Info,
            title: "No reclaimable artifacts found".to_owned(),
            message: "The current scan did not find any eligible cleanup candidates.".to_owned(),
            category: None,
            workspace_index: None,
            candidate_count: 0,
            bytes: 0,
            occurrences: 0,
        })
    } else if history.scan_count < 2 {
        Some(AnalysisInsight {
            kind: InsightKind::HistoryBaseline,
            severity: InsightSeverity::Info,
            title: "Building local history baseline".to_owned(),
            message: "Run analyze again after future scans to unlock repeated-growth insights."
                .to_owned(),
            category: None,
            workspace_index: None,
            candidate_count: report.candidates.len(),
            bytes: report.total_bytes,
            occurrences: history.scan_count,
        })
    } else if otherwise_stable {
        Some(AnalysisInsight {
            kind: InsightKind::Stable,
            severity: InsightSeverity::Info,
            title: "No actionable trend detected".to_owned(),
            message:
                "Current artifacts and aggregate history are within the configured thresholds."
                    .to_owned(),
            category: None,
            workspace_index: None,
            candidate_count: report.candidates.len(),
            bytes: report.total_bytes,
            occurrences: 0,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::{Candidate, Confidence};

    fn history() -> HistorySummary {
        HistorySummary {
            days: 30,
            scan_count: 3,
            cleanup_count: 0,
            latest_reclaimable_bytes: 4096,
            reclaimable_change_bytes: 2048,
            removed_bytes: 0,
            quarantined_bytes: 0,
            failures: 0,
            category_change_bytes: BTreeMap::new(),
            category_growth_events: BTreeMap::new(),
        }
    }

    fn report(modified_at_unix: u64) -> ScanReport {
        ScanReport {
            roots: vec![PathBuf::from("/project")],
            candidates: vec![Candidate {
                category: Category::RustTarget,
                path: PathBuf::from("/project/target"),
                bytes: 4096,
                reason: "test".to_owned(),
                modified_at_unix: Some(modified_at_unix),
                confidence: Confidence::Safe,
                approved_rule: None,
                custom_rule: None,
            }],
            review_candidates: Vec::new(),
            learning_observations: Vec::new(),
            workspaces: Vec::new(),
            warnings: Vec::new(),
            total_bytes: 4096,
            review_total_bytes: 0,
            observed_total_bytes: 0,
            protect_git_tracked: true,
        }
    }

    #[test]
    fn stale_artifacts_should_be_grouped_without_paths() {
        let output = analyze_at(&report(1), &history(), 60, 100 * 86_400);

        assert_eq!(output.insights[0].kind, InsightKind::StaleArtifacts);
        assert_eq!(output.insights[0].bytes, 4096);
        assert!(!output.insights[0].message.contains("/project"));
    }

    #[test]
    fn aggregate_growth_events_should_become_actionable_insight() {
        let mut history = history();
        history
            .category_growth_events
            .insert(Category::NodeModules, 4);
        history
            .category_change_bytes
            .insert(Category::NodeModules, 8192);

        let output = analyze_at(&report(99 * 86_400), &history, 60, 100 * 86_400);

        assert!(output.insights.iter().any(|insight| {
            insight.kind == InsightKind::RepeatedGrowth && insight.occurrences == 4
        }));
    }
}
