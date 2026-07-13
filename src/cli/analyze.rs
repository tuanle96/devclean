use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args as ClapArgs, ValueEnum};
use devclean::{AnalysisReport, InsightSeverity, analyze, human_bytes, load_config, scan};

use super::{
    SharedScanArgs, history_database_override, record_scan_history, scan_options, select_categories,
};

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Rolling aggregate history window used for trend detection.
    #[arg(long, default_value_t = 30)]
    days: u64,

    /// Treat artifacts unchanged for at least this many days as stale.
    #[arg(long, default_value_t = 60)]
    stale_after_days: u64,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Table)]
    format: Format,

    /// Write output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Replace workspace roots with stable placeholders.
    #[arg(long)]
    redact_paths: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Table,
    Json,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    if arguments.days == 0 {
        bail!("--days must be greater than zero");
    }
    if arguments.stale_after_days == 0 {
        bail!("--stale-after-days must be greater than zero");
    }

    let config = load_config(arguments.shared.config.as_deref())?;
    let categories = select_categories(&arguments.shared, true, false);
    let scan_report = scan(&scan_options(
        &arguments.shared,
        &config,
        categories,
        false,
    )?)?;
    record_scan_history(&scan_report);
    let database = history_database_override();
    let history = devclean::history::summarize(arguments.days, database.as_deref())?;
    let mut report = analyze(&scan_report, &history, arguments.stale_after_days);
    if arguments.redact_paths {
        redact_workspace_paths(&mut report);
    }
    let output = match arguments.format {
        Format::Table => render_table(&report),
        Format::Json => serde_json::to_string_pretty(&report)?,
    };
    write_output(&output, arguments.output.as_deref())
}

fn redact_workspace_paths(report: &mut AnalysisReport) {
    for (index, workspace) in report.workspaces.iter_mut().enumerate() {
        workspace.root = PathBuf::from(format!("<workspace:{}>", index + 1));
    }
}

fn render_table(report: &AnalysisReport) -> String {
    let mut output = format!(
        "devclean analysis\ncurrent: {} across {} candidates\nhistory: {} scans and {} cleanups over {} days\nchange: {}\n",
        human_bytes(report.current_reclaimable_bytes),
        report.candidate_count,
        report.history_scan_count,
        report.history_cleanup_count,
        report.days,
        signed_bytes(report.reclaimable_change_bytes),
    );
    if !report.workspaces.is_empty() {
        let _ = writeln!(output, "workspaces:");
        for (index, workspace) in report.workspaces.iter().enumerate() {
            let kinds = workspace
                .kinds
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("+");
            let _ = writeln!(
                output,
                "  {}. {kinds} · {} · {} candidates · {}",
                index + 1,
                human_bytes(workspace.total_bytes),
                workspace.candidate_count,
                workspace.root.display()
            );
        }
    }
    let _ = writeln!(output, "insights:");
    for insight in &report.insights {
        let _ = writeln!(
            output,
            "  [{}] {} · {}",
            severity_label(insight.severity),
            insight.title,
            human_bytes(insight.bytes)
        );
        let _ = writeln!(output, "      {}", insight.message);
    }
    output
}

const fn severity_label(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Opportunity => "opportunity",
        InsightSeverity::Warning => "warning",
    }
}

fn signed_bytes(value: i64) -> String {
    let magnitude = value.unsigned_abs();
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => format!("+{}", human_bytes(magnitude)),
        std::cmp::Ordering::Less => format!("-{}", human_bytes(magnitude)),
        std::cmp::Ordering::Equal => "0 B".to_owned(),
    }
}

fn write_output(output: &str, path: Option<&Path>) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
        }
        fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        println!("{output}");
    }
    Ok(())
}
