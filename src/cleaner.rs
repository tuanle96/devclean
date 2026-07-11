use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow, bail};
use directories::BaseDirs;
use serde::Serialize;

use crate::model::{Candidate, Category, ScanReport};
use crate::policy::contains_git_tracked_files;
use crate::quarantine::{QuarantineEntry, hold};
use crate::scanner::{
    classify, classify_approved_review_candidate, expensive_global_cache_paths, global_cache_paths,
};

static QUARANTINE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Outcome of deleting a validated scan report.
#[derive(Debug, Serialize)]
pub struct CleanReport {
    /// Candidates removed successfully.
    pub removed: Vec<Candidate>,
    /// Candidates moved into persistent safety holds instead of being deleted.
    pub quarantined: Vec<QuarantineEntry>,
    /// Candidate-specific failures. Cleanup continues after a failure.
    pub failures: Vec<String>,
    /// Scan-time allocated bytes represented by successful removals.
    pub removed_bytes: u64,
    /// Bytes retained on disk until their safety holds are purged.
    pub quarantined_bytes: u64,
}

/// Controls whether validated candidates are deleted immediately or held for restoration.
#[derive(Debug, Clone, Default)]
pub struct CleanOptions {
    /// Retain candidates for this duration. `None` preserves immediate cleanup behavior.
    pub quarantine_for: Option<Duration>,
    /// Override the platform quarantine registry, primarily for isolated automation.
    pub quarantine_registry: Option<PathBuf>,
}

/// Removes only candidates that still satisfy the scan-time safety policy.
#[must_use]
pub fn clean(scan_report: &ScanReport) -> CleanReport {
    clean_with_options(scan_report, &CleanOptions::default())
}

/// Processes validated candidates using explicit retention options.
#[must_use]
pub fn clean_with_options(scan_report: &ScanReport, options: &CleanOptions) -> CleanReport {
    let mut removed = Vec::new();
    let mut quarantined = Vec::new();
    let mut failures = Vec::new();
    let allowed_global = BaseDirs::new().map_or_else(Vec::new, |base| {
        let mut paths = global_cache_paths(base.home_dir());
        paths.extend(expensive_global_cache_paths(base.home_dir()));
        paths
    });

    for candidate in &scan_report.candidates {
        if let Err(error) = validate_candidate(
            candidate,
            &scan_report.roots,
            &allowed_global,
            scan_report.protect_git_tracked,
        ) {
            failures.push(format!("{}: {error}", candidate.path.display()));
            continue;
        }
        if let Some(retention) = options.quarantine_for {
            match hold(
                &candidate.path,
                candidate.category,
                candidate.bytes,
                retention,
                options.quarantine_registry.as_deref(),
            ) {
                Ok(entry) => quarantined.push(entry),
                Err(error) => failures.push(format!("{}: {error:#}", candidate.path.display())),
            }
        } else {
            match quarantine_and_remove(&candidate.path) {
                Ok(()) => removed.push(candidate.clone()),
                Err(error) => failures.push(format!("{}: {error:#}", candidate.path.display())),
            }
        }
    }

    let removed_bytes = removed.iter().map(|candidate| candidate.bytes).sum();
    let quarantined_bytes = quarantined.iter().map(|entry| entry.bytes).sum();
    CleanReport {
        removed,
        quarantined,
        failures,
        removed_bytes,
        quarantined_bytes,
    }
}

fn validate_candidate(
    candidate: &Candidate,
    roots: &[PathBuf],
    allowed_global: &[PathBuf],
    protect_git_tracked: bool,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(&candidate.path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("candidate is no longer a real directory".to_owned());
    }

    if matches!(
        candidate.category,
        Category::GlobalCache | Category::ExpensiveGlobalCache
    ) {
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

    let current = candidate.approved_rule.map_or_else(
        || classify(&candidate.path),
        |rule| classify_approved_review_candidate(&candidate.path, rule),
    );
    let Some((current_category, _)) = current else {
        return Err("candidate no longer matches a rebuildable artifact".to_owned());
    };
    if current_category != candidate.category {
        return Err("candidate category changed after scanning".to_owned());
    }
    if protect_git_tracked {
        match contains_git_tracked_files(&candidate.path) {
            Ok(true) => return Err("candidate now contains Git-tracked files".to_owned()),
            Ok(false) => {}
            Err(error) => return Err(format!("Git tracked-file guard failed: {error:#}")),
        }
    }
    Ok(())
}

fn quarantine_and_remove(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("candidate has no parent directory"))?;
    let sequence = QUARANTINE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let quarantine = parent.join(format!(
        ".devclean-quarantine-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    if quarantine.exists() {
        bail!("unique quarantine path unexpectedly exists");
    }
    fs::rename(path, &quarantine)?;

    let metadata = fs::symlink_metadata(&quarantine)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        let _ = fs::rename(&quarantine, path);
        bail!("candidate changed type during atomic quarantine");
    }
    if let Err(error) = fs::remove_dir_all(&quarantine) {
        let restored = fs::rename(&quarantine, path).is_ok();
        bail!(
            "failed to remove quarantined directory: {error}; restored original path: {restored}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::scanner::{LearningMode, ScanOptions, scan};

    fn options(root: &Path, category: Category) -> ScanOptions {
        ScanOptions {
            roots: vec![root.to_path_buf()],
            categories: HashSet::from([category]),
            include_global_caches: false,
            include_expensive_caches: false,
            max_depth: 8,
            excludes: Vec::new(),
            older_than: None,
            min_size: 0,
            protect_git_tracked: false,
            learning_mode: LearningMode::Disabled,
            approved_review_paths: HashSet::new(),
        }
    }

    #[test]
    fn clean_should_remove_scanned_node_modules() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(&modules)?;
        fs::write(modules.join("dependency.js"), "content")?;
        let report = scan(&options(temporary.path(), Category::NodeModules))?;

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
        let report = scan(&options(temporary.path(), Category::RustTarget))?;
        fs::remove_dir_all(target.join("debug"))?;

        let clean_report = clean(&report);

        assert_eq!(clean_report.failures.len(), 1);
        Ok(())
    }

    #[test]
    fn clean_should_leave_no_quarantine_after_success() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(&modules)?;
        let report = scan(&options(temporary.path(), Category::NodeModules))?;

        let _ = clean(&report);
        let leftovers = temporary
            .path()
            .read_dir()?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".devclean-quarantine")
            })
            .count();

        assert_eq!(leftovers, 0);
        Ok(())
    }
}
