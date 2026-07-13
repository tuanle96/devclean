use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use devclean::docker;
use devclean::{OutputFormat, load_config, scan};

use super::{SharedScanArgs, record_scan_history, scan_options, select_categories, write_report};

#[derive(Debug, Args)]
pub(super) struct ScanArgs {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Accepted for symmetry with clean; scan already includes every rebuildable category.
    #[arg(long)]
    all: bool,

    /// Include Docker's detailed, read-only disk usage summary.
    #[arg(long)]
    docker: bool,

    /// Report format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Replace absolute paths in reports with root-relative placeholders.
    #[arg(long)]
    redact_paths: bool,

    #[command(flatten)]
    learning: LearningArgs,
}

#[derive(Debug, Args)]
struct LearningArgs {
    /// Observe large cache-like directories as review-only Learning Mode candidates.
    #[arg(long)]
    learning: bool,
}

pub(super) fn run(arguments: &ScanArgs) -> Result<()> {
    let config = load_config(arguments.shared.config.as_deref())?;
    let categories = select_categories(&arguments.shared, arguments.all, false);
    let report = scan(&scan_options(
        &arguments.shared,
        &config,
        categories,
        arguments.learning.learning,
    )?)?;
    record_scan_history(&report);
    write_report(
        &report,
        arguments.format,
        arguments.output.as_deref(),
        arguments.redact_paths,
    )?;
    if arguments.docker {
        println!("\nDocker disk usage:\n{}", docker::system_df()?);
    }
    Ok(())
}
