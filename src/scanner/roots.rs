use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::BaseDirs;

// Keep this list in sync with DefaultScanLocations.relativePaths in the macOS app.
const DEFAULT_ROOT_CANDIDATES: [&str; 13] = [
    "Dev",
    "Developer",
    "Projects",
    "Code",
    "src",
    "workspace",
    "Workspaces",
    "Repos",
    "Repositories",
    "GitHub",
    "Documents/GitHub",
    "AndroidStudioProjects",
    "IdeaProjects",
];

/// Returns useful development roots without scanning the entire home directory.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots =
        BaseDirs::new().map_or_else(Vec::new, |base| discover_default_roots(base.home_dir()));
    if roots.is_empty() {
        if let Ok(current) = env::current_dir() {
            roots.push(current);
        }
    }
    roots
}

fn discover_default_roots(home: &Path) -> Vec<PathBuf> {
    DEFAULT_ROOT_CANDIDATES
        .iter()
        .map(|relative| home.join(relative))
        .filter(|candidate| candidate.is_dir())
        .collect()
}

pub(super) fn normalize_roots(
    roots: &[PathBuf],
    warnings: &mut Vec<String>,
) -> Result<Vec<PathBuf>> {
    let mut normalized: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !root.is_dir() {
            warnings.push(format!("skipped missing root: {}", root.display()));
            continue;
        }
        let canonical = root
            .canonicalize()
            .with_context(|| format!("failed to normalize root {}", root.display()))?;
        if normalized
            .iter()
            .any(|existing| canonical == *existing || canonical.starts_with(existing))
        {
            continue;
        }
        normalized.retain(|existing| !existing.starts_with(&canonical));
        normalized.push(canonical);
    }
    if normalized.is_empty() {
        anyhow::bail!("no valid scan roots were found");
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn discovery_should_include_existing_conventions_only() -> Result<()> {
        let temporary = tempdir()?;
        for relative in [
            "Dev",
            "workspace",
            "Documents/GitHub",
            "Library/Developer/CoreSimulator",
        ] {
            fs::create_dir_all(temporary.path().join(relative))?;
        }

        assert_eq!(
            discover_default_roots(temporary.path()),
            [
                temporary.path().join("Dev"),
                temporary.path().join("workspace"),
                temporary.path().join("Documents/GitHub"),
            ]
        );
        Ok(())
    }

    #[test]
    fn normalization_should_remove_duplicate_and_nested_roots() -> Result<()> {
        let temporary = tempdir()?;
        let parent = temporary.path().join("Projects");
        let child = parent.join("nested");
        fs::create_dir_all(&child)?;
        let mut warnings = Vec::new();

        let normalized = normalize_roots(&[child, parent.clone(), parent.clone()], &mut warnings)?;

        assert_eq!(normalized, [parent.canonicalize()?]);
        assert!(warnings.is_empty());
        Ok(())
    }

    #[test]
    fn normalization_should_warn_and_reject_missing_roots() {
        let temporary = tempdir().expect("temporary directory");
        let missing = temporary.path().join("missing");
        let mut warnings = Vec::new();

        assert!(normalize_roots(std::slice::from_ref(&missing), &mut warnings).is_err());
        assert_eq!(
            warnings,
            [format!("skipped missing root: {}", missing.display())]
        );
    }
}
