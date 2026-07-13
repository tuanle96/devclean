use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::{Args as ClapArgs, Subcommand};
use uuid::Uuid;

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(subcommand)]
    command: CommandArgs,
}

#[derive(Debug, Subcommand)]
enum CommandArgs {
    /// Clone a Git repository, validate one config file, and copy it locally.
    Fetch(FetchArgs),
}

#[derive(Debug, ClapArgs)]
struct FetchArgs {
    /// Git repository URL or local repository path.
    repository: String,
    /// Config path inside the repository.
    #[arg(long, default_value = ".devclean.toml")]
    file: PathBuf,
    /// Validated local destination.
    #[arg(long, default_value = ".devclean.toml")]
    output: PathBuf,
    /// Replace an existing destination.
    #[arg(long)]
    force: bool,
    /// Print the fetch plan without cloning or writing.
    #[arg(long)]
    dry_run: bool,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    match &arguments.command {
        CommandArgs::Fetch(arguments) => fetch(arguments),
    }
}

fn fetch(arguments: &FetchArgs) -> Result<()> {
    validate_relative_file(&arguments.file)?;
    if arguments.output.exists() && !arguments.force {
        bail!(
            "{} already exists; pass --force to replace it",
            arguments.output.display()
        );
    }
    if arguments.dry_run {
        println!(
            "would clone {} and validate {} before writing {}",
            arguments.repository,
            arguments.file.display(),
            arguments.output.display()
        );
        return Ok(());
    }

    let checkout = std::env::temp_dir().join(format!("devclean-config-{}", Uuid::new_v4()));
    let result = fetch_into(arguments, &checkout);
    let _ = fs::remove_dir_all(&checkout);
    result
}

fn fetch_into(arguments: &FetchArgs, checkout: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["clone", "--depth", "1", "--"])
        .arg(&arguments.repository)
        .arg(checkout)
        .stdin(Stdio::null())
        .status()
        .context("failed to run git clone for shared config")?;
    if !status.success() {
        bail!("git clone failed for shared config repository");
    }
    let source = checkout.join(&arguments.file);
    devclean::load_config(Some(&source)).context("shared config did not pass strict validation")?;
    if let Some(parent) = arguments
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &arguments.output).with_context(|| {
        format!(
            "failed to copy validated config to {}",
            arguments.output.display()
        )
    })?;
    println!(
        "installed validated config at {}",
        arguments.output.display()
    );
    Ok(())
}

fn validate_relative_file(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "--file must be a repository-relative path without parent traversal"
    );
    Ok(())
}
