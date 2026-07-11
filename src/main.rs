use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use devclean::docker;
use devclean::render::{human_bytes, render};
use devclean::{Category, OutputFormat, ScanOptions, clean, default_roots, scan};

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
    /// Show defaults, safety guarantees, and tool availability.
    Doctor,
}

#[derive(Debug, Args)]
struct SharedScanArgs {
    /// Roots to scan. Defaults to ~/Dev and ~/Documents/Codex when present.
    #[arg(value_name = "ROOT")]
    roots: Vec<PathBuf>,

    /// Categories to include. May be repeated or comma-separated.
    #[arg(long, value_enum, value_delimiter = ',')]
    category: Vec<Category>,

    /// Include downloaded package-manager and development-tool caches.
    #[arg(long)]
    global_caches: bool,

    /// Maximum directory depth below each root.
    #[arg(long, default_value_t = 24)]
    max_depth: usize,
}

#[derive(Debug, Args)]
struct ScanArgs {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Include every rebuildable category, including build and test outputs.
    #[arg(long)]
    all: bool,

    /// Include Docker's read-only disk usage summary.
    #[arg(long)]
    docker: bool,

    /// Report format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CleanArgs {
    #[command(flatten)]
    shared: SharedScanArgs,

    /// Include build and test output categories in addition to safe defaults.
    #[arg(long)]
    all: bool,

    /// Also prune stopped containers, unused images/networks, and build cache.
    /// Docker volumes are never removed.
    #[arg(long)]
    docker: bool,

    /// Skip the interactive DELETE confirmation.
    #[arg(long)]
    yes: bool,

    /// Save the pre-clean scan as a standalone HTML report.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Scan(arguments) => run_scan(&arguments),
        Commands::Clean(arguments) => run_clean(&arguments),
        Commands::Doctor => {
            run_doctor();
            Ok(())
        }
    }
}

fn run_scan(arguments: &ScanArgs) -> Result<()> {
    let categories = select_categories(&arguments.shared.category, arguments.all, false);
    let report = scan(&scan_options(&arguments.shared, categories))?;
    write_report(&report, arguments.format, arguments.output.as_deref())?;
    if arguments.docker {
        println!("\nDocker disk usage:\n{}", docker::system_df()?);
    }
    Ok(())
}

fn run_clean(arguments: &CleanArgs) -> Result<()> {
    let categories = select_categories(&arguments.shared.category, arguments.all, true);
    let report = scan(&scan_options(&arguments.shared, categories))?;
    print!("{}", render(&report, OutputFormat::Table)?);

    if let Some(path) = &arguments.report {
        write_report(&report, OutputFormat::Html, Some(path))?;
        println!("pre-clean report: {}", path.display());
    }
    if report.candidates.is_empty() && !arguments.docker {
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
    if arguments.docker {
        println!("{}", docker::prune()?);
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
    println!("  symlinks, VCS metadata, backups, PostgreSQL and volumes are protected");
    println!("  Docker prune never includes --volumes");
}

fn scan_options(arguments: &SharedScanArgs, categories: HashSet<Category>) -> ScanOptions {
    ScanOptions {
        roots: if arguments.roots.is_empty() {
            default_roots()
        } else {
            arguments.roots.clone()
        },
        categories,
        include_global_caches: arguments.global_caches,
        max_depth: arguments.max_depth,
    }
}

fn select_categories(
    explicit: &[Category],
    all: bool,
    conservative_default: bool,
) -> HashSet<Category> {
    if !explicit.is_empty() {
        return explicit.iter().copied().collect();
    }
    if all || !conservative_default {
        Category::all().into_iter().collect()
    } else {
        Category::safe_defaults().into_iter().collect()
    }
}

fn write_report(
    report: &devclean::ScanReport,
    format: OutputFormat,
    path: Option<&Path>,
) -> Result<()> {
    let rendered = render(report, format)?;
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
