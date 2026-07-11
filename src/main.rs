use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use devclean::docker;
use devclean::{
    Category, Config, OutputFormat, RenderOptions, ScanOptions, ScanReport, clean,
    config_candidates, default_roots, human_bytes, load_config, parse_age, parse_bytes,
    render_with_options, scan,
};

#[derive(Debug, Parser)]
#[command(
    name = "devclean",
    version,
    about = "Audit and safely remove rebuildable development artifacts",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Inventory rebuildable artifacts without deleting anything.
    Scan(ScanArgs),
    /// Delete a freshly scanned, safety-validated cleanup plan.
    Clean(CleanArgs),
    /// Show defaults, safety guarantees, configuration, and tool availability.
    Doctor,
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
    /// Generate a roff manual page.
    Manpage(ManpageArgs),
}

#[derive(Debug, Args)]
struct SharedScanArgs {
    /// Roots to scan. Defaults to config, then common development directories.
    #[arg(value_name = "ROOT")]
    roots: Vec<PathBuf>,

    /// Configuration file. Defaults to ./devclean.toml or the platform config directory.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Categories to include. May be repeated or comma-separated.
    #[arg(long, value_enum, value_delimiter = ',')]
    category: Vec<Category>,

    /// Exclude a path glob. May be repeated.
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Include package-manager and development-tool caches that are cheap to restore.
    #[arg(long)]
    global_caches: bool,

    /// Include large runtime and model caches that can be expensive to restore.
    #[arg(long)]
    expensive_caches: bool,

    /// Only include artifacts older than this duration, for example 30d or 12h.
    #[arg(long)]
    older_than: Option<String>,

    /// Only include artifacts at least this large, for example 500MiB.
    #[arg(long)]
    min_size: Option<String>,

    /// Maximum directory depth below each root.
    #[arg(long)]
    max_depth: Option<usize>,

    /// Permit cleanup of candidates containing Git-tracked files.
    #[arg(long)]
    allow_tracked: bool,
}

#[derive(Debug, Args)]
struct ScanArgs {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Include every rebuildable category, including build and test outputs.
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
}

#[derive(Debug, Args)]
struct CleanArgs {
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

    #[command(flatten)]
    report: CleanReportArgs,
}

#[derive(Debug, Args)]
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

#[derive(Debug, Args)]
struct SelectionArgs {
    /// Interactively select candidates by number or range.
    #[arg(long)]
    select: bool,

    /// Remove only enough candidates to reach this amount of free space.
    #[arg(long, value_name = "SIZE")]
    target_free: Option<String>,
}

#[derive(Debug, Args)]
struct CleanReportArgs {
    /// Save the exact pre-clean plan as a standalone HTML report.
    #[arg(long)]
    report: Option<PathBuf>,

    /// Replace absolute paths in the saved report with root-relative placeholders.
    #[arg(long)]
    redact_paths: bool,
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell syntax to generate.
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Debug, Args)]
struct ManpageArgs {
    /// Output path. Defaults to stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Scan(arguments) => run_scan(&arguments),
        Commands::Clean(arguments) => run_clean(&arguments),
        Commands::Doctor => {
            run_doctor();
            Ok(())
        }
        Commands::Completions(arguments) => {
            clap_complete::generate(
                arguments.shell,
                &mut Cli::command(),
                "devclean",
                &mut io::stdout(),
            );
            Ok(())
        }
        Commands::Manpage(arguments) => run_manpage(arguments.output.as_deref()),
    }
}

fn run_scan(arguments: &ScanArgs) -> Result<()> {
    let config = load_config(arguments.shared.config.as_deref())?;
    let categories = select_categories(&arguments.shared, arguments.all, false);
    let report = scan(&scan_options(&arguments.shared, &config, categories)?)?;
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

fn run_clean(arguments: &CleanArgs) -> Result<()> {
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
    let mut report = scan(&scan_options(&arguments.shared, &config, categories)?)?;
    if let Some(target) = arguments.selection.target_free.as_deref() {
        report = limit_to_target_free(report, parse_bytes(target)?)?;
    }
    if arguments.selection.select && !report.candidates.is_empty() {
        report = select_candidates(report)?;
    }

    print!(
        "{}",
        render_with_options(&report, OutputFormat::Table, RenderOptions::default())?
    );
    if let Some(path) = &arguments.report.report {
        write_report(
            &report,
            OutputFormat::Html,
            Some(path),
            arguments.report.redact_paths,
        )?;
        println!("pre-clean report: {}", path.display());
    }

    let docker_requested = arguments.docker.docker || arguments.docker.docker_system;
    if report.candidates.is_empty() && !docker_requested {
        println!("nothing to clean");
        return Ok(());
    }
    confirm(arguments.yes)?;

    let cleaned = clean(&report);
    for candidate in &cleaned.removed {
        println!(
            "removed {:>10}  {}",
            human_bytes(candidate.bytes),
            candidate.path.display()
        );
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
    println!(
        "removed {} filesystem candidates, {} estimated",
        cleaned.removed.len(),
        human_bytes(cleaned.removed_bytes)
    );
    if !cleaned.failures.is_empty() {
        for failure in &cleaned.failures {
            eprintln!("failed: {failure}");
        }
        bail!("{} candidates could not be removed", cleaned.failures.len());
    }
    Ok(())
}

fn run_doctor() {
    println!("devclean {}", env!("CARGO_PKG_VERSION"));
    println!("default roots:");
    for root in default_roots() {
        println!("  {}", root.display());
    }
    println!("config search:");
    for path in config_candidates() {
        println!(
            "  {} {}",
            if path.is_file() {
                "loaded"
            } else {
                "candidate"
            },
            path.display()
        );
    }
    println!("tools:");
    for tool in ["cargo", "docker", "git", "npm", "pnpm"] {
        println!(
            "  {tool:<8} {}",
            if command_exists(tool) {
                "available"
            } else {
                "not found"
            }
        );
    }
    println!("safety:");
    println!("  scan is always read-only");
    println!("  clean requires confirmation or --yes");
    println!("  Git-tracked files are protected unless --allow-tracked is explicit");
    println!("  candidates are atomically quarantined before recursive deletion");
    println!("  symlinks, VCS metadata, backups, databases and volumes are protected");
    println!("  --docker prunes build cache only; --docker-system never includes volumes");
}

fn scan_options(
    arguments: &SharedScanArgs,
    config: &Config,
    categories: HashSet<Category>,
) -> Result<ScanOptions> {
    let roots = if !arguments.roots.is_empty() {
        arguments
            .roots
            .iter()
            .map(|path| expand_root(path))
            .collect()
    } else if !config.scan.roots.is_empty() {
        config
            .scan
            .roots
            .iter()
            .map(|path| expand_root(path))
            .collect()
    } else {
        default_roots()
    };
    let mut excludes = config.scan.exclude.clone();
    excludes.extend(arguments.exclude.iter().cloned());
    let older_than = arguments
        .older_than
        .as_deref()
        .or(config.scan.older_than.as_deref())
        .map(parse_age)
        .transpose()?;
    let min_size = arguments
        .min_size
        .as_deref()
        .or(config.scan.min_size.as_deref())
        .map(parse_bytes)
        .transpose()?
        .unwrap_or(0);

    Ok(ScanOptions {
        roots,
        categories,
        include_global_caches: arguments.global_caches,
        include_expensive_caches: arguments.expensive_caches || config.clean.expensive_caches,
        max_depth: arguments.max_depth.or(config.scan.max_depth).unwrap_or(24),
        excludes,
        older_than,
        min_size,
        protect_git_tracked: config.clean.protect_git_tracked && !arguments.allow_tracked,
    })
}

fn select_categories(
    arguments: &SharedScanArgs,
    all: bool,
    conservative_default: bool,
) -> HashSet<Category> {
    let mut categories: HashSet<Category> = if !arguments.category.is_empty() {
        arguments.category.iter().copied().collect()
    } else if all || !conservative_default {
        Category::all().into_iter().collect()
    } else {
        Category::safe_defaults().into_iter().collect()
    };
    if arguments.global_caches {
        categories.insert(Category::GlobalCache);
    }
    if arguments.expensive_caches {
        categories.insert(Category::ExpensiveGlobalCache);
    }
    categories
}

fn expand_root(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(relative) = value.strip_prefix("~/") {
        return directories::BaseDirs::new()
            .map_or_else(|| path.to_path_buf(), |base| base.home_dir().join(relative));
    }
    path.to_path_buf()
}

fn limit_to_target_free(mut report: ScanReport, target: u64) -> Result<ScanReport> {
    let root = report
        .roots
        .first()
        .context("target-free requires at least one scan root")?;
    let available = fs2::available_space(root)?;
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

fn write_report(
    report: &ScanReport,
    format: OutputFormat,
    path: Option<&Path>,
    redact_paths: bool,
) -> Result<()> {
    let rendered = render_with_options(report, format, RenderOptions { redact_paths })?;
    if let Some(path) = path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
        }
        fs::write(path, rendered).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn run_manpage(path: Option<&Path>) -> Result<()> {
    let man = clap_mangen::Man::new(Cli::command());
    if let Some(path) = path {
        let mut file = fs::File::create(path)
            .with_context(|| format!("failed to create {}", path.display()))?;
        man.render(&mut file)?;
    } else {
        man.render(&mut io::stdout())?;
    }
    Ok(())
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

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
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
}
