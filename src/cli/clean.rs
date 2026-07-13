use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use devclean::docker;
use devclean::{
    Category, CleanOptions, OutputFormat, RenderOptions, ScanReport, clean_with_options,
    human_bytes, load_config, parse_age, parse_bytes, render_with_options, restore_quarantine,
    scan,
};

use super::quarantine::format_expiry;
use super::{
    SharedScanArgs, expand_root, record_cleanup_history, scan_options, select_categories,
    write_report,
};

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Include build and test output categories in addition to safe defaults.
    #[arg(long)]
    all: bool,

    #[command(flatten)]
    docker: DockerArgs,

    #[command(flatten)]
    selection: SelectionArgs,

    /// Skip the final DELETE confirmation.
    #[arg(long)]
    yes: bool,

    /// Print the fully filtered cleanup plan without deleting or quarantining anything.
    #[arg(long, conflicts_with = "undo")]
    dry_run: bool,

    /// Restore one safety hold by ID; shorthand for `quarantine restore <ID>`.
    #[arg(long, value_name = "ID", conflicts_with_all = ["dry_run", "select", "target_free", "docker", "docker_system"])]
    undo: Option<String>,

    /// Keep selected artifacts restorable for this duration, for example 7d.
    #[arg(long, value_name = "DURATION")]
    quarantine_for: Option<String>,

    /// Override the quarantine registry path.
    #[arg(long, hide = true)]
    quarantine_registry: Option<PathBuf>,

    #[command(flatten)]
    report: CleanReportArgs,
}

#[derive(Debug, ClapArgs)]
struct DockerArgs {
    /// Prune only unused Docker build cache.
    #[arg(long, conflicts_with = "docker_system")]
    docker: bool,

    /// Prune stopped containers, unused images/networks, and build cache. Never volumes.
    #[arg(long, conflicts_with = "docker")]
    docker_system: bool,

    /// Pass an `until` filter to Docker prune, for example 168h.
    #[arg(long)]
    docker_older_than: Option<String>,
}

#[derive(Debug, ClapArgs)]
struct SelectionArgs {
    /// Interactively select candidates by number or range.
    #[arg(long)]
    select: bool,

    /// Clean only exact candidate paths from a previous JSON scan. May be repeated.
    #[arg(
        long = "only-path",
        value_name = "PATH",
        conflicts_with_all = ["select", "target_free"]
    )]
    only_paths: Vec<PathBuf>,

    /// Remove only enough candidates to reach this amount of free space.
    #[arg(long, value_name = "SIZE")]
    target_free: Option<String>,
}

#[derive(Debug, ClapArgs)]
struct CleanReportArgs {
    /// Save the exact pre-clean plan as a standalone HTML report.
    #[arg(long)]
    report: Option<PathBuf>,

    /// Replace absolute paths in the saved report with root-relative placeholders.
    #[arg(long)]
    redact_paths: bool,
    /// Emit one machine-readable cleanup outcome line for schedulers.
    #[arg(long)]
    result_jsonl: bool,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    if let Some(id) = &arguments.undo {
        let entry = restore_quarantine(id, arguments.quarantine_registry.as_deref())?;
        println!("restored {}", entry.original_path.display());
        return Ok(());
    }
    if arguments.docker.docker_older_than.is_some()
        && !arguments.docker.docker
        && !arguments.docker.docker_system
    {
        bail!("--docker-older-than requires --docker or --docker-system");
    }
    let config = load_config(arguments.shared.config.as_deref())?;
    let mut categories = select_categories(&arguments.shared, arguments.all, true);
    if config.clean.expensive_caches {
        categories.insert(Category::ExpensiveGlobalCache);
    }
    let mut report = scan(&scan_options(
        &arguments.shared,
        &config,
        categories,
        false,
    )?)?;
    if let Some(target) = arguments.selection.target_free.as_deref() {
        report = limit_to_target_free(report, parse_bytes(target)?)?;
    }
    if !arguments.selection.only_paths.is_empty() {
        report = select_exact_candidates(report, &arguments.selection.only_paths)?;
    }
    if arguments.selection.select && !report.candidates.is_empty() {
        report = select_candidates(report)?;
    }

    if !arguments.report.result_jsonl {
        render_clean_plan(&report, &arguments.report)?;
    }

    let docker_requested = arguments.docker.docker || arguments.docker.docker_system;
    if arguments.dry_run {
        report_dry_run(&report, &arguments.docker);
        return Ok(());
    }
    if report.candidates.is_empty() && !docker_requested {
        if arguments.report.result_jsonl {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "type": "cleanup",
                    "removed": [],
                    "quarantined": [],
                    "failures": [],
                    "removed_bytes": 0,
                    "quarantined_bytes": 0,
                }))?
            );
        } else {
            println!("nothing to clean");
        }
        return Ok(());
    }
    confirm(arguments.yes)?;

    let clean_options = CleanOptions {
        quarantine_for: arguments
            .quarantine_for
            .as_deref()
            .map(parse_age)
            .transpose()?,
        quarantine_registry: arguments.quarantine_registry.clone(),
    };
    let cleaned = clean_with_options(&report, &clean_options);
    record_cleanup_history(&cleaned);
    if arguments.report.result_jsonl {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({"type": "cleanup", "outcome": &cleaned}))?
        );
    } else {
        print_clean_outcome(&cleaned);
    }
    if arguments.docker.docker {
        println!(
            "{}",
            docker::prune_build_cache(arguments.docker.docker_older_than.as_deref())?
        );
    } else if arguments.docker.docker_system {
        println!(
            "{}",
            docker::prune_system(arguments.docker.docker_older_than.as_deref())?
        );
    }
    if !cleaned.failures.is_empty() {
        for failure in &cleaned.failures {
            eprintln!("failed: {failure}");
        }
        bail!("{} candidates could not be removed", cleaned.failures.len());
    }
    Ok(())
}

fn print_clean_outcome(cleaned: &devclean::CleanReport) {
    for candidate in &cleaned.removed {
        println!(
            "removed {:>10}  {}",
            human_bytes(candidate.bytes),
            candidate.path.display()
        );
    }
    for entry in &cleaned.quarantined {
        println!(
            "held    {:>10}  {}  until {}",
            human_bytes(entry.bytes),
            entry.original_path.display(),
            format_expiry(entry.expires_at_unix)
        );
    }
    println!(
        "removed {} filesystem candidates, {} estimated",
        cleaned.removed.len(),
        human_bytes(cleaned.removed_bytes)
    );
    if !cleaned.quarantined.is_empty() {
        println!(
            "held {} candidates, {} retained on disk until purge",
            cleaned.quarantined.len(),
            human_bytes(cleaned.quarantined_bytes)
        );
    }
}

fn render_clean_plan(report: &ScanReport, options: &CleanReportArgs) -> Result<()> {
    print!(
        "{}",
        render_with_options(report, OutputFormat::Table, RenderOptions::default())?
    );
    if let Some(path) = &options.report {
        write_report(report, OutputFormat::Html, Some(path), options.redact_paths)?;
        println!("pre-clean report: {}", path.display());
    }
    Ok(())
}

fn report_dry_run(report: &ScanReport, docker: &DockerArgs) {
    println!(
        "dry run: planned {} filesystem candidates, {}; no changes made",
        report.candidates.len(),
        human_bytes(report.total_bytes)
    );
    if docker.docker {
        println!("dry run: Docker build cache prune requested; Docker was not invoked");
    } else if docker.docker_system {
        println!("dry run: Docker system prune requested; Docker was not invoked");
    }
}

fn limit_to_target_free(mut report: ScanReport, target: u64) -> Result<ScanReport> {
    let root = report
        .roots
        .first()
        .context("target-free requires at least one scan root")?;
    let available = fs4::available_space(root)?;
    let needed = target.saturating_sub(available);
    if needed == 0 {
        report.candidates.clear();
        report.total_bytes = 0;
        return Ok(report);
    }
    let mut selected = Vec::new();
    let mut selected_bytes = 0_u64;
    for candidate in report.candidates {
        selected_bytes = selected_bytes.saturating_add(candidate.bytes);
        selected.push(candidate);
        if selected_bytes >= needed {
            break;
        }
    }
    report.candidates = selected;
    report.total_bytes = selected_bytes;
    Ok(report)
}

fn select_candidates(mut report: ScanReport) -> Result<ScanReport> {
    if !io::stdin().is_terminal() {
        bail!("--select requires an interactive terminal");
    }
    for (index, candidate) in report.candidates.iter().enumerate() {
        println!(
            "{:>4}. {:>10}  {}",
            index + 1,
            human_bytes(candidate.bytes),
            candidate.path.display()
        );
    }
    print!("Select candidates (example: 1,3-5 or all): ");
    io::stdout().flush()?;
    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    let indexes = parse_selection(response.trim(), report.candidates.len())?;
    report.candidates = report
        .candidates
        .into_iter()
        .enumerate()
        .filter_map(|(index, candidate)| indexes.contains(&(index + 1)).then_some(candidate))
        .collect();
    report.total_bytes = report
        .candidates
        .iter()
        .map(|candidate| candidate.bytes)
        .sum();
    Ok(report)
}

fn select_exact_candidates(
    mut report: ScanReport,
    requested_paths: &[PathBuf],
) -> Result<ScanReport> {
    let requested: HashSet<PathBuf> = requested_paths
        .iter()
        .map(|path| path_identity(path))
        .collect();
    let found: HashSet<PathBuf> = report
        .candidates
        .iter()
        .map(|candidate| path_identity(&candidate.path))
        .filter(|path| requested.contains(path))
        .collect();
    let mut missing: Vec<_> = requested.difference(&found).collect();
    missing.sort();
    if let Some(path) = missing.first() {
        bail!(
            "selected path is no longer an eligible cleanup candidate: {}",
            path.display()
        );
    }

    report
        .candidates
        .retain(|candidate| requested.contains(&path_identity(&candidate.path)));
    report.total_bytes = report
        .candidates
        .iter()
        .map(|candidate| candidate.bytes)
        .sum();
    Ok(report)
}

fn path_identity(path: &Path) -> PathBuf {
    let expanded = expand_root(path);
    fs::canonicalize(&expanded).unwrap_or(expanded)
}

fn parse_selection(value: &str, maximum: usize) -> Result<HashSet<usize>> {
    if value.eq_ignore_ascii_case("all") {
        return Ok((1..=maximum).collect());
    }
    let mut selected = HashSet::new();
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some((start, end)) = part.split_once('-') {
            let start = start.parse::<usize>()?;
            let end = end.parse::<usize>()?;
            if start == 0 || start > end || end > maximum {
                bail!("selection range `{part}` is outside 1..={maximum}");
            }
            selected.extend(start..=end);
        } else {
            let index = part.parse::<usize>()?;
            if index == 0 || index > maximum {
                bail!("selection `{index}` is outside 1..={maximum}");
            }
            selected.insert(index);
        }
    }
    if selected.is_empty() {
        bail!("no candidates selected");
    }
    Ok(selected)
}

fn confirm(assume_yes: bool) -> Result<()> {
    if assume_yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        bail!("refusing non-interactive cleanup without --yes");
    }
    print!("Type DELETE to remove the listed artifacts: ");
    io::stdout().flush()?;
    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    if response.trim() != "DELETE" {
        bail!("cleanup cancelled");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_selection_should_accept_ranges() -> Result<()> {
        let selected = parse_selection("1,3-5", 5)?;

        assert_eq!(selected, HashSet::from([1, 3, 4, 5]));
        Ok(())
    }

    #[test]
    fn parse_selection_should_reject_out_of_range_value() {
        assert!(parse_selection("6", 5).is_err());
    }

    #[test]
    fn exact_selection_should_reject_stale_path() {
        let report = ScanReport {
            roots: vec![PathBuf::from("/tmp/project")],
            candidates: Vec::new(),
            review_candidates: Vec::new(),
            learning_observations: Vec::new(),
            workspaces: Vec::new(),
            warnings: Vec::new(),
            total_bytes: 0,
            review_total_bytes: 0,
            observed_total_bytes: 0,
            protect_git_tracked: true,
        };

        assert!(
            select_exact_candidates(report, &[PathBuf::from("/tmp/project/node_modules")]).is_err()
        );
    }
}
