use std::collections::HashSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use devclean::{
    Category, Config, LearningMode, OutputFormat, RenderOptions, ScanOptions, ScanReport,
    default_roots, parse_age, parse_bytes, render_with_options,
};

mod clean;
mod config_command;
mod doctor;
mod init;
mod quarantine;
mod scan;
mod schedule;
mod stats;
mod tui;
mod watch;

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
    /// Generate a documented project configuration template.
    Init(init::Args),
    /// Fetch and validate a shared configuration from a Git repository.
    Config(config_command::Args),
    /// Show local scan growth and cleanup history.
    Stats(stats::Args),
    /// Install, inspect, or remove recurring cleanup jobs.
    Schedule(schedule::Args),
    /// Select cleanup candidates in a read-only terminal UI.
    Tui(tui::Args),
    /// Inventory rebuildable artifacts without deleting anything.
    Scan(scan::ScanArgs),
    /// Watch scan roots and report when reclaimable artifacts cross a threshold.
    Watch(WatchArgs),
    /// Delete a freshly scanned, safety-validated cleanup plan.
    Clean(clean::Args),
    /// Show defaults, safety guarantees, configuration, and tool availability.
    Doctor,
    /// List, restore, or purge persistent cleanup safety holds.
    Quarantine(quarantine::Args),
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
    /// Generate a roff manual page.
    Manpage(ManpageArgs),
}

#[derive(Debug, Args)]
struct WatchArgs {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Notify when reclaimable artifacts reach this size, for example 5GiB.
    #[arg(long, value_name = "SIZE")]
    threshold: Option<String>,

    /// Minimum duration between event-triggered scans, for example 1h.
    #[arg(long, value_name = "DURATION")]
    interval: Option<String>,

    /// Scan once and exit; useful for launch agents and verification.
    #[arg(long)]
    once: bool,

    /// Print threshold crossings without sending a desktop notification.
    #[arg(long)]
    no_notify: bool,
}

#[derive(Debug, Args)]
struct SharedScanArgs {
    /// Roots to scan. Defaults to config, then common development directories.
    #[arg(value_name = "ROOT")]
    roots: Vec<PathBuf>,

    /// Configuration file. Defaults to ./.devclean.toml, ./devclean.toml, or platform config.
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

    /// Approve an exact review path only when it still matches a scanner-owned safe rule.
    #[arg(long = "approve-review-path", value_name = "PATH")]
    approved_review_paths: Vec<PathBuf>,
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

pub fn run() -> Result<()> {
    match Cli::parse().command {
        Commands::Init(arguments) => init::run(&arguments),
        Commands::Config(arguments) => config_command::run(&arguments),
        Commands::Stats(arguments) => stats::run(&arguments),
        Commands::Schedule(arguments) => schedule::run(&arguments),
        Commands::Tui(arguments) => tui::run(&arguments),
        Commands::Scan(arguments) => scan::run(&arguments),
        Commands::Watch(arguments) => watch::run(&arguments),
        Commands::Clean(arguments) => clean::run(&arguments),
        Commands::Doctor => {
            doctor::run();
            Ok(())
        }
        Commands::Quarantine(arguments) => quarantine::run(&arguments),
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

fn scan_options(
    arguments: &SharedScanArgs,
    config: &Config,
    categories: HashSet<Category>,
    learning_mode: bool,
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
        learning_mode: if learning_mode {
            LearningMode::Enabled
        } else {
            LearningMode::Disabled
        },
        approved_review_paths: arguments.approved_review_paths.iter().cloned().collect(),
        custom_rules: config.rules.clone(),
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

fn history_database_override() -> Option<PathBuf> {
    env::var_os("DEVCLEAN_HISTORY_DB").map(PathBuf::from)
}

fn history_recording_enabled() -> bool {
    !cfg!(debug_assertions) || env::var_os("DEVCLEAN_HISTORY_IN_DEBUG").is_some()
}

fn record_scan_history(report: &ScanReport) {
    if history_recording_enabled() {
        if let Err(error) =
            devclean::history::record_scan(report, history_database_override().as_deref())
        {
            eprintln!("warning: failed to record scan history: {error:#}");
        }
    }
}

fn record_cleanup_history(report: &devclean::CleanReport) {
    if history_recording_enabled() {
        if let Err(error) =
            devclean::history::record_cleanup(report, history_database_override().as_deref())
        {
            eprintln!("warning: failed to record cleanup history: {error:#}");
        }
    }
}
