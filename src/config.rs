use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::Deserialize;

use crate::model::CustomRule;

/// User configuration loaded from `devclean.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Read-only discovery settings.
    pub scan: ScanConfig,
    /// Destructive cleanup settings.
    pub clean: CleanConfig,
    /// Read-only filesystem watcher settings.
    pub watch: WatchConfig,
    /// Declarative project-local scanner extensions.
    pub rules: Vec<CustomRule>,
}

/// Settings for the read-only filesystem watcher.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatchConfig {
    /// Notify when reclaimable artifacts reach this size.
    pub threshold: String,
    /// Minimum duration between event-triggered scans.
    pub interval: String,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            threshold: "5GiB".to_owned(),
            interval: "1h".to_owned(),
        }
    }
}

/// Settings that affect candidate discovery.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScanConfig {
    /// Default roots when the CLI does not supply roots.
    pub roots: Vec<PathBuf>,
    /// Glob patterns excluded from traversal and cleanup.
    pub exclude: Vec<String>,
    /// Only include artifacts older than this duration, for example `30d`.
    pub older_than: Option<String>,
    /// Only include artifacts at least this large, for example `1GiB`.
    pub min_size: Option<String>,
    /// Maximum traversal depth.
    pub max_depth: Option<usize>,
}

/// Settings that affect destructive cleanup.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CleanConfig {
    /// Refuse candidates that contain Git-tracked files.
    pub protect_git_tracked: bool,
    /// Include large model and runtime caches that are expensive to restore.
    pub expensive_caches: bool,
}

impl Default for CleanConfig {
    fn default() -> Self {
        Self {
            protect_git_tracked: true,
            expensive_caches: false,
        }
    }
}

/// Returns configuration locations in precedence order.
#[must_use]
pub fn config_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current) = env::current_dir() {
        candidates.push(current.join(".devclean.toml"));
        candidates.push(current.join("devclean.toml"));
    }
    if let Some(base) = BaseDirs::new() {
        candidates.push(base.config_dir().join("devclean/config.toml"));
    }
    candidates
}

/// Loads an explicit configuration file or the first discovered default.
///
/// # Errors
///
/// Returns an error when an explicit file is missing, unreadable, or invalid.
pub fn load_config(explicit: Option<&Path>) -> Result<Config> {
    let selected = if let Some(path) = explicit {
        Some(path.to_path_buf())
    } else {
        config_candidates().into_iter().find(|path| path.is_file())
    };
    let Some(path) = selected else {
        return Ok(Config::default());
    };
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("invalid config {}", path.display()))?;
    validate_custom_rules(&config.rules)?;
    Ok(config)
}

fn validate_custom_rules(rules: &[CustomRule]) -> Result<()> {
    for rule in rules {
        anyhow::ensure!(
            !rule.name.trim().is_empty(),
            "custom rule name cannot be empty"
        );
        anyhow::ensure!(
            !rule.directory_names.is_empty(),
            "custom rule `{}` needs at least one directory name",
            rule.name
        );
        anyhow::ensure!(
            !rule.required_markers.is_empty(),
            "custom rule `{}` needs at least one direct marker",
            rule.name
        );
        for value in rule.directory_names.iter().chain(&rule.required_markers) {
            let candidate = Path::new(value);
            anyhow::ensure!(
                candidate.components().count() == 1
                    && matches!(
                        candidate.components().next(),
                        Some(std::path::Component::Normal(_))
                    ),
                "custom rule `{}` contains unsafe name `{value}`",
                rule.name
            );
        }
    }
    Ok(())
}

/// Parses a human duration such as `12h`, `30d`, or `3weeks`.
///
/// # Errors
///
/// Returns an error for an invalid duration.
pub fn parse_age(value: &str) -> Result<Duration> {
    humantime::parse_duration(value).with_context(|| format!("invalid duration `{value}`"))
}

/// Parses a human byte size such as `500MB` or `2GiB`.
///
/// # Errors
///
/// Returns an error for an invalid byte size.
pub fn parse_bytes(value: &str) -> Result<u64> {
    parse_size::parse_size(value).with_context(|| format!("invalid byte size `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_age_should_accept_days() -> Result<()> {
        assert_eq!(parse_age("30d")?, Duration::from_secs(30 * 86_400));
        Ok(())
    }

    #[test]
    fn parse_bytes_should_accept_binary_units() -> Result<()> {
        assert_eq!(parse_bytes("2GiB")?, 2 * 1024 * 1024 * 1024);
        Ok(())
    }

    #[test]
    fn custom_rules_should_require_exact_names_and_direct_markers() {
        let invalid = CustomRule {
            name: "unsafe".to_owned(),
            category: crate::model::Category::BuildOutput,
            directory_names: vec!["../dist".to_owned()],
            required_markers: vec!["package.json".to_owned()],
            reason: "generated output".to_owned(),
        };

        assert!(validate_custom_rules(&[invalid]).is_err());
    }
}
