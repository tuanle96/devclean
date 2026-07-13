use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    /// Destination configuration file.
    #[arg(long, default_value = ".devclean.toml")]
    output: PathBuf,
    /// Replace an existing destination.
    #[arg(long)]
    force: bool,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    if arguments.output.exists() && !arguments.force {
        bail!(
            "{} already exists; pass --force to replace it",
            arguments.output.display()
        );
    }
    if let Some(parent) = arguments
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(
        &arguments.output,
        include_str!("../../devclean.example.toml"),
    )
    .with_context(|| format!("failed to write {}", arguments.output.display()))?;
    println!("created {}", arguments.output.display());
    Ok(())
}
