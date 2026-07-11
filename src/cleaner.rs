use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::model::{Candidate, Category, ScanReport};
use crate::scanner::{classify, global_cache_paths};

/// Result of a cleanup operation.
#[derive(Debug, Clone, Serialize)]
pub struct CleanReport {
    /// Successfully removed candidates.
    pub removed: Vec<Candidate>,
    /// Paths that failed safety validation or deletion.
    pub failures: Vec<String>,
    /// Estimated bytes removed successfully.
    pub removed_bytes: u64,
}

/// Removes only candidates that still satisfy the scan-time safety policy.
#[must_use]
pub fn clean(scan_report: &ScanReport) -> CleanReport {
    let mut removed = Vec::new();
    let mut failures = Vec::new();
    let home = env::var_os("HOME").map(PathBuf::from);
    let allowed_global = home.as_deref().map(global_cache_paths).unwrap_or_default();

    for candidate in &scan_report.candidates {
        if let Err(error) = validate_candidate(candidate, &scan_report.roots, &allowed_global) {
            failures.push(format!("{}: {error}", candidate.path.display()));
            continue;
        }
        match fs::remove_dir_all(&candidate.path) {
            Ok(()) => removed.push(candidate.clone()),
            Err(error) => failures.push(format!("{}: {error}", candidate.path.display())),
        }
    }

    let removed_bytes = removed.iter().map(|candidate| candidate.bytes).sum();
    CleanReport {
        removed,
        failures,
        removed_bytes,
    }
}

fn validate_candidate(
    candidate: &Candidate,
    roots: &[PathBuf],
    allowed_global: &[PathBuf],
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(&candidate.path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("candidate is no longer a real directory".to_owned());
    }

    if candidate.category == Category::GlobalCache {
        if allowed_global.iter().any(|path| path == &candidate.path) {
            return Ok(());
        }
        return Err("global cache is not on the exact allowlist".to_owned());
    }

    let canonical = candidate
        .path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !roots.iter().any(|root| canonical.starts_with(root)) {
        return Err("candidate escaped the configured roots".to_owned());
    }

    let Some((current_category, _)) = classify(&candidate.path) else {
        return Err("candidate no longer matches a rebuildable artifact".to_owned());
    };
    if current_category != candidate.category {
        return Err("candidate category changed after scanning".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use anyhow::Result;
    use tempfile::tempdir;

    use super::*;
    use crate::scanner::{ScanOptions, scan};

    #[test]
    fn clean_should_remove_scanned_node_modules() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(&modules)?;
        fs::write(modules.join("dependency.js"), "content")?;
        let report = scan(&ScanOptions {
            roots: vec![temporary.path().to_path_buf()],
            categories: HashSet::from([Category::NodeModules]),
            include_global_caches: false,
            max_depth: 8,
        })?;

        let clean_report = clean(&report);

        assert!(!modules.exists());
        assert!(clean_report.failures.is_empty());
        Ok(())
    }

    #[test]
    fn clean_should_reject_candidate_that_changed_category() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target");
        fs::create_dir_all(target.join("debug"))?;
        let report = scan(&ScanOptions {
            roots: vec![temporary.path().to_path_buf()],
            categories: HashSet::from([Category::RustTarget]),
            include_global_caches: false,
            max_depth: 8,
        })?;
        fs::remove_dir_all(target.join("debug"))?;

        let clean_report = clean(&report);

        assert_eq!(clean_report.failures.len(), 1);
        Ok(())
    }
}
