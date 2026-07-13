use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args as ClapArgs, ValueEnum};
use devclean::history::HistorySummary;
use minijinja::{Environment, context};

use super::history_database_override;

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    /// Rolling history window.
    #[arg(long, default_value_t = 30)]
    days: u64,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Table)]
    format: Format,
    /// Write output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
    /// Override the history database.
    #[arg(long, hide = true)]
    database: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Table,
    Json,
    Html,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    if arguments.days == 0 {
        bail!("--days must be greater than zero");
    }
    let environment_database = history_database_override();
    let database = arguments
        .database
        .as_deref()
        .or(environment_database.as_deref());
    let summary = devclean::history::summarize(arguments.days, database)?;
    let output = match arguments.format {
        Format::Table => render_table(&summary),
        Format::Json => serde_json::to_string_pretty(&summary)?,
        Format::Html => render_html(&summary)?,
    };
    if let Some(path) = &arguments.output {
        fs::write(path, &output).with_context(|| format!("failed to write {}", path.display()))?;
        println!("wrote {}", path.display());
    } else {
        println!("{output}");
    }
    Ok(())
}

fn render_table(summary: &HistorySummary) -> String {
    let mut output = format!(
        "{}-day history\nscans: {}\ncleanups: {}\nlatest reclaimable: {}\nchange: {}\nremoved: {}\nheld: {}\nfailures: {}\n",
        summary.days,
        summary.scan_count,
        summary.cleanup_count,
        devclean::human_bytes(summary.latest_reclaimable_bytes),
        signed_bytes(summary.reclaimable_change_bytes),
        devclean::human_bytes(summary.removed_bytes),
        devclean::human_bytes(summary.quarantined_bytes),
        summary.failures,
    );
    for (category, bytes) in &summary.category_change_bytes {
        let _ = writeln!(output, "{category}: {}", signed_bytes(*bytes));
    }
    output
}

fn render_html(summary: &HistorySummary) -> Result<String> {
    let mut environment = Environment::new();
    environment.add_template(
        "stats.html",
        include_str!("../../templates/stats-report.html"),
    )?;
    let categories = summary
        .category_change_bytes
        .iter()
        .map(|(category, bytes)| (category.to_string(), signed_bytes(*bytes)))
        .collect::<Vec<_>>();
    Ok(environment.get_template("stats.html")?.render(context! {
        days => summary.days,
        scan_count => summary.scan_count,
        cleanup_count => summary.cleanup_count,
        latest => devclean::human_bytes(summary.latest_reclaimable_bytes),
        change => signed_bytes(summary.reclaimable_change_bytes),
        removed => devclean::human_bytes(summary.removed_bytes),
        held => devclean::human_bytes(summary.quarantined_bytes),
        failures => summary.failures,
        categories,
    })?)
}

fn signed_bytes(value: i64) -> String {
    let magnitude = value.unsigned_abs();
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => format!("+{}", devclean::human_bytes(magnitude)),
        std::cmp::Ordering::Less => format!("-{}", devclean::human_bytes(magnitude)),
        std::cmp::Ordering::Equal => "0 B".to_owned(),
    }
}
