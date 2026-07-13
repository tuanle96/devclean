use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use clap::{Args as ClapArgs, Subcommand};
use devclean::{human_bytes, list_quarantine, purge_expired, purge_selected, restore_quarantine};

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List restorable safety holds.
    List(ListArgs),
    /// Restore one safety hold to its original path.
    Restore(RestoreArgs),
    /// Permanently delete expired safety holds.
    Purge(PurgeArgs),
}

#[derive(Debug, ClapArgs)]
struct ListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
    /// Override the quarantine registry path.
    #[arg(long, hide = true)]
    registry: Option<PathBuf>,
}

#[derive(Debug, ClapArgs)]
struct RestoreArgs {
    /// Quarantine identifier shown by `quarantine list`.
    id: String,
    /// Override the quarantine registry path.
    #[arg(long, hide = true)]
    registry: Option<PathBuf>,
}

#[derive(Debug, ClapArgs)]
struct PurgeArgs {
    /// Purge all holds, including those that have not expired.
    #[arg(long)]
    all: bool,
    /// Permanently delete one exact safety hold before or after expiry.
    #[arg(long, value_name = "ID", conflicts_with = "all")]
    id: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
    /// Override the quarantine registry path.
    #[arg(long, hide = true)]
    registry: Option<PathBuf>,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    match &arguments.command {
        Commands::List(arguments) => {
            let entries = list_quarantine(arguments.registry.as_deref())?;
            if arguments.json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("no safety holds");
            } else {
                println!("ID\tEXPIRES\tSIZE\tORIGINAL PATH");
                for entry in entries {
                    println!(
                        "{}\t{}\t{}\t{}",
                        entry.id,
                        format_expiry(entry.expires_at_unix),
                        human_bytes(entry.bytes),
                        entry.original_path.display()
                    );
                }
            }
            Ok(())
        }
        Commands::Restore(arguments) => {
            let entry = restore_quarantine(&arguments.id, arguments.registry.as_deref())?;
            println!("restored {}", entry.original_path.display());
            Ok(())
        }
        Commands::Purge(arguments) => {
            let report = if let Some(id) = &arguments.id {
                purge_selected(id, arguments.registry.as_deref())?
            } else {
                let now = if arguments.all {
                    u64::MAX
                } else {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |duration| duration.as_secs())
                };
                purge_expired(now, arguments.registry.as_deref())?
            };
            if arguments.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "purged {} safety holds, {} reclaimed",
                    report.purged.len(),
                    human_bytes(report.purged_bytes)
                );
                for failure in &report.failures {
                    eprintln!("failed: {failure}");
                }
            }
            if !report.failures.is_empty() {
                bail!("{} safety holds could not be purged", report.failures.len());
            }
            Ok(())
        }
    }
}

pub(super) fn format_expiry(expires_at_unix: u64) -> String {
    // humantime cannot render RFC 3339 beyond year 9999 and would panic through Display.
    const RFC3339_MAX_SECONDS: u64 = 253_402_300_800;
    if expires_at_unix >= RFC3339_MAX_SECONDS {
        return expires_at_unix.to_string();
    }
    UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(expires_at_unix))
        .map_or_else(
            || expires_at_unix.to_string(),
            |timestamp| humantime::format_rfc3339_seconds(timestamp).to_string(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_expiry_should_render_rfc3339() {
        assert_eq!(format_expiry(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn format_expiry_should_fall_back_to_seconds_beyond_year_9999() {
        assert_eq!(format_expiry(u64::MAX), u64::MAX.to_string());
        assert_eq!(format_expiry(253_402_300_800), "253402300800");
    }
}
