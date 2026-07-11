use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use directories::BaseDirs;
use walkdir::WalkDir;

use crate::model::{
    Candidate, Category, Confidence, LearningObservation, ReviewCandidate, ReviewRule, ScanReport,
};
use crate::policy::{ExcludePolicy, contains_git_tracked_files};

/// Scanner configuration.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Filesystem roots to inspect.
    pub roots: Vec<PathBuf>,
    /// Artifact categories to include.
    pub categories: HashSet<Category>,
    /// Include known global package and tool caches.
    pub include_global_caches: bool,
    /// Include large runtime and model caches.
    pub include_expensive_caches: bool,
    /// Maximum traversal depth below each root.
    pub max_depth: usize,
    /// User-provided path globs to skip.
    pub excludes: Vec<String>,
    /// Only include artifacts whose latest modification is at least this old.
    pub older_than: Option<Duration>,
    /// Only include artifacts at least this large.
    pub min_size: u64,
    /// Refuse candidates containing Git-tracked files.
    pub protect_git_tracked: bool,
    /// Observe large cache-like directories without making them cleanable.
    pub learning_mode: LearningMode,
    /// Exact review paths approved through a scanner-recognized learning rule.
    pub approved_review_paths: HashSet<PathBuf>,
}

/// Controls whether the scanner emits local growth observations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LearningMode {
    /// Return cleanup candidates only.
    #[default]
    Disabled,
    /// Measure active known artifacts and review-only cache-like directories.
    Enabled,
}

impl LearningMode {
    const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[derive(Debug, Default)]
struct ScanAccumulator {
    candidates: Vec<Candidate>,
    review_candidates: Vec<ReviewCandidate>,
    learning_observations: Vec<LearningObservation>,
    warnings: Vec<String>,
}

/// Returns useful development roots without scanning the entire home directory.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(base) = BaseDirs::new() {
        for relative in ["Dev", "Projects", "Documents/Codex"] {
            let candidate = base.home_dir().join(relative);
            if candidate.is_dir() {
                roots.push(candidate);
            }
        }
    }
    if roots.is_empty() {
        if let Ok(current) = env::current_dir() {
            roots.push(current);
        }
    }
    roots
}

/// Scans configured roots without modifying the filesystem.
///
/// # Errors
///
/// Returns an error when no valid root exists, a root cannot be normalized, or an exclude glob
/// is invalid.
pub fn scan(options: &ScanOptions) -> Result<ScanReport> {
    let mut output = ScanAccumulator::default();
    let normalized_roots = normalize_roots(&options.roots, &mut output.warnings)?;

    let mut effective_options = options.clone();
    effective_options.approved_review_paths = options
        .approved_review_paths
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect();

    let excludes = ExcludePolicy::new(&options.excludes)?;
    for root in &normalized_roots {
        scan_root(
            root,
            &normalized_roots,
            &effective_options,
            &excludes,
            &mut output,
        );
    }

    if effective_options.include_global_caches
        && effective_options
            .categories
            .contains(&Category::GlobalCache)
    {
        add_global_cache_candidates(
            Category::GlobalCache,
            global_cache_paths,
            &normalized_roots,
            &effective_options,
            &excludes,
            &mut output,
        );
    }
    if effective_options.include_expensive_caches
        && effective_options
            .categories
            .contains(&Category::ExpensiveGlobalCache)
    {
        add_global_cache_candidates(
            Category::ExpensiveGlobalCache,
            expensive_global_cache_paths,
            &normalized_roots,
            &effective_options,
            &excludes,
            &mut output,
        );
    }

    output.candidates.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    let total_bytes = output
        .candidates
        .iter()
        .map(|candidate| candidate.bytes)
        .fold(0_u64, u64::saturating_add);
    output.review_candidates.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    let review_total_bytes = output
        .review_candidates
        .iter()
        .map(|candidate| candidate.bytes)
        .fold(0_u64, u64::saturating_add);
    output.learning_observations.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    let observed_total_bytes = output
        .learning_observations
        .iter()
        .map(|observation| observation.bytes)
        .fold(0_u64, u64::saturating_add);

    Ok(ScanReport {
        roots: normalized_roots,
        candidates: output.candidates,
        review_candidates: output.review_candidates,
        learning_observations: output.learning_observations,
        warnings: output.warnings,
        total_bytes,
        review_total_bytes,
        observed_total_bytes,
        protect_git_tracked: effective_options.protect_git_tracked,
    })
}

fn normalize_roots(roots: &[PathBuf], warnings: &mut Vec<String>) -> Result<Vec<PathBuf>> {
    let mut normalized = Vec::new();
    for root in roots {
        if !root.is_dir() {
            warnings.push(format!("skipped missing root: {}", root.display()));
            continue;
        }
        normalized.push(
            root.canonicalize()
                .with_context(|| format!("failed to normalize root {}", root.display()))?,
        );
    }
    if normalized.is_empty() {
        anyhow::bail!("no valid scan roots were found");
    }
    Ok(normalized)
}

fn scan_root(
    root: &Path,
    roots: &[PathBuf],
    options: &ScanOptions,
    excludes: &ExcludePolicy,
    output: &mut ScanAccumulator,
) {
    let mut walker = WalkDir::new(root)
        .max_depth(options.max_depth)
        .follow_links(false)
        .same_file_system(true)
        .into_iter();

    while let Some(entry_result) = walker.next() {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                output.warnings.push(error.to_string());
                continue;
            }
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        if path != root && (should_prune(path) || excludes.matches(path, roots)) {
            walker.skip_current_dir();
            continue;
        }

        let Some((category, reason)) = classify(path) else {
            if options.learning_mode.is_enabled() || options.approved_review_paths.contains(path) {
                if let Some(reason) = classify_review_candidate(path) {
                    walker.skip_current_dir();
                    add_review_candidate(path, reason, options, output);
                }
            }
            continue;
        };
        walker.skip_current_dir();
        add_candidate(
            path,
            category,
            reason,
            options.categories.contains(&category),
            options,
            output,
        );
    }
}

fn add_candidate(
    path: &Path,
    category: Category,
    reason: &'static str,
    include_cleanup_candidate: bool,
    options: &ScanOptions,
    output: &mut ScanAccumulator,
) {
    let stats = match artifact_stats(path) {
        Ok(stats) => stats,
        Err(error) => {
            output
                .warnings
                .push(format!("{}: {error:#}", path.display()));
            return;
        }
    };
    if options.protect_git_tracked {
        match contains_git_tracked_files(path) {
            Ok(true) => {
                if options.learning_mode.is_enabled() {
                    output.learning_observations.push(LearningObservation {
                        path: path.to_path_buf(),
                        category: Some(category),
                        bytes: stats.bytes,
                        reason: "contains Git-tracked files".to_owned(),
                        modified_at_unix: stats.modified.and_then(system_time_to_unix),
                        confidence: Confidence::Protected,
                    });
                }
                output.warnings.push(format!(
                    "protected Git-tracked candidate: {}",
                    path.display()
                ));
                return;
            }
            Ok(false) => {}
            Err(error) => {
                output.warnings.push(format!(
                    "skipped {} because tracked-file guard failed: {error:#}",
                    path.display()
                ));
                return;
            }
        }
    }
    if options.learning_mode.is_enabled() {
        output.learning_observations.push(LearningObservation {
            path: path.to_path_buf(),
            category: Some(category),
            bytes: stats.bytes,
            reason: reason.to_owned(),
            modified_at_unix: stats.modified.and_then(system_time_to_unix),
            confidence: Confidence::Safe,
        });
    }
    if !include_cleanup_candidate
        || stats.bytes < options.min_size
        || !is_old_enough(stats.modified, options.older_than)
    {
        return;
    }
    output.candidates.push(Candidate {
        category,
        path: path.to_path_buf(),
        bytes: stats.bytes,
        reason: reason.to_owned(),
        modified_at_unix: stats.modified.and_then(system_time_to_unix),
        confidence: Confidence::Safe,
        approved_rule: None,
    });
}

fn add_review_candidate(
    path: &Path,
    reason: &'static str,
    options: &ScanOptions,
    output: &mut ScanAccumulator,
) {
    let stats = match artifact_stats(path) {
        Ok(stats) => stats,
        Err(error) => {
            output
                .warnings
                .push(format!("{}: {error:#}", path.display()));
            return;
        }
    };
    if stats.bytes < options.min_size {
        return;
    }
    if options.protect_git_tracked {
        match contains_git_tracked_files(path) {
            Ok(true) => {
                output.learning_observations.push(LearningObservation {
                    path: path.to_path_buf(),
                    category: None,
                    bytes: stats.bytes,
                    reason: "contains Git-tracked files".to_owned(),
                    modified_at_unix: stats.modified.and_then(system_time_to_unix),
                    confidence: Confidence::Protected,
                });
                output.warnings.push(format!(
                    "protected Git-tracked learning candidate: {}",
                    path.display()
                ));
                return;
            }
            Ok(false) => {}
            Err(error) => {
                output.warnings.push(format!(
                    "skipped learning candidate {} because tracked-file guard failed: {error:#}",
                    path.display()
                ));
                return;
            }
        }
    }
    let suggestion = suggested_review_rule(path);
    let approved_rule = suggestion
        .as_ref()
        .map(|(rule, _)| *rule)
        .filter(|_| options.approved_review_paths.contains(path));
    let approved = approved_rule.is_some();
    let category = approved.then_some(Category::BuildOutput);
    output.learning_observations.push(LearningObservation {
        path: path.to_path_buf(),
        category,
        bytes: stats.bytes,
        reason: reason.to_owned(),
        modified_at_unix: stats.modified.and_then(system_time_to_unix),
        confidence: if approved {
            Confidence::Safe
        } else {
            Confidence::Review
        },
    });
    if let Some(rule) = approved_rule {
        if stats.bytes >= options.min_size && is_old_enough(stats.modified, options.older_than) {
            let Some((category, approved_reason)) = classify_approved_review_candidate(path, rule)
            else {
                return;
            };
            output.candidates.push(Candidate {
                category,
                path: path.to_path_buf(),
                bytes: stats.bytes,
                reason: approved_reason.to_owned(),
                modified_at_unix: stats.modified.and_then(system_time_to_unix),
                confidence: Confidence::Safe,
                approved_rule: Some(rule),
            });
            return;
        }
    }
    output.review_candidates.push(ReviewCandidate {
        path: path.to_path_buf(),
        bytes: stats.bytes,
        reason: reason.to_owned(),
        modified_at_unix: stats.modified.and_then(system_time_to_unix),
        confidence: Confidence::Review,
        suggested_rule: suggestion.as_ref().map(|(rule, _)| *rule),
        project_root: suggestion.map(|(_, root)| root),
        approved,
    });
}

fn suggested_review_rule(path: &Path) -> Option<(ReviewRule, PathBuf)> {
    let parent = path.parent()?;
    classify_approved_review_candidate(path, ReviewRule::SwiftPackageBuild)
        .map(|_| (ReviewRule::SwiftPackageBuild, parent.to_path_buf()))
}

/// Revalidates a user-approved review path against its scanner-owned rule.
#[must_use]
pub fn classify_approved_review_candidate(
    path: &Path,
    rule: ReviewRule,
) -> Option<(Category, &'static str)> {
    if is_protected(path) {
        return None;
    }
    match rule {
        ReviewRule::SwiftPackageBuild => {
            let parent = path.parent()?;
            (path.file_name() == Some(OsStr::new(".build"))
                && parent.join("Package.swift").is_file())
            .then_some((
                Category::BuildOutput,
                "user-approved Swift Package build directory",
            ))
        }
    }
}

/// Classifies a directory using conservative, filesystem-verifiable markers.
#[must_use]
pub fn classify(path: &Path) -> Option<(Category, &'static str)> {
    if is_protected(path) {
        return None;
    }
    let name = path.file_name()?;

    if name == OsStr::new("target") && looks_like_rust_target(path) {
        return Some((
            Category::RustTarget,
            "Cargo target directory with Rust build markers",
        ));
    }
    if matches_name(name, &["node_modules", "frontend_node_modules"]) {
        return Some((Category::NodeModules, "installed JavaScript dependencies"));
    }
    if matches_name(
        name,
        &[
            ".next",
            ".svelte-kit",
            ".turbo",
            ".vite",
            ".parcel-cache",
            ".nuxt",
            ".output",
            ".dart_tool",
            ".npm-cache",
        ],
    ) {
        return Some((Category::FrameworkCache, "framework-generated cache"));
    }
    if matches_name(
        name,
        &[
            "mutants.out",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".nyc_output",
        ],
    ) {
        return Some((Category::TestCache, "test or analysis cache"));
    }
    if name == OsStr::new("build") && looks_like_project_build(path) {
        return Some((
            Category::BuildOutput,
            "build directory beneath a recognized project",
        ));
    }
    None
}

fn classify_review_candidate(path: &Path) -> Option<&'static str> {
    if is_protected(path) || !has_project_marker(path) {
        return None;
    }
    let name = path.file_name()?;
    if matches_name(
        name,
        &[
            ".build",
            ".cache",
            ".gradle",
            ".angular",
            ".expo",
            "DerivedData",
            "Pods",
            "cache",
            "coverage",
            "dist",
            "generated",
            "out",
            "temp",
            "tmp",
        ],
    ) {
        return Some("large cache-like directory beneath a recognized project");
    }
    None
}

fn has_project_marker(path: &Path) -> bool {
    path.ancestors().skip(1).take(3).any(|ancestor| {
        [
            "Cargo.toml",
            "Package.swift",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pubspec.yaml",
            "build.gradle",
            "settings.gradle",
        ]
        .iter()
        .any(|marker| ancestor.join(marker).is_file())
            || ancestor
                .read_dir()
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .any(|entry| entry.path().extension() == Some(OsStr::new("xcodeproj")))
    })
}

fn should_prune(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches_name(name, &[".git", ".hg", ".svn", ".venv", "site-packages"])
            || name.to_string_lossy().starts_with(".devclean-quarantine-")
    })
}

fn looks_like_rust_target(path: &Path) -> bool {
    path.join("CACHEDIR.TAG").is_file()
        || path.join(".rustc_info.json").is_file()
        || path.join("debug").is_dir()
        || path.join("release").is_dir()
}

fn looks_like_project_build(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if ["package.json", "pubspec.yaml", "Cargo.toml"]
        .iter()
        .any(|marker| parent.join(marker).is_file())
    {
        return true;
    }
    parent.file_name() == Some(OsStr::new("ios"))
        || parent
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension() == Some(OsStr::new("xcodeproj")))
}

fn is_protected(path: &Path) -> bool {
    path.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        let value = name.to_string_lossy();
        [
            ".git",
            ".hg",
            ".svn",
            "backups",
            "backup",
            "volumes",
            "postgres",
            "postgresql",
            "mysql",
            "mariadb",
            "filestore",
        ]
        .iter()
        .any(|protected| value.eq_ignore_ascii_case(protected))
    })
}

fn matches_name(name: &OsStr, values: &[&str]) -> bool {
    values.iter().any(|value| name == OsStr::new(value))
}

fn add_global_cache_candidates(
    category: Category,
    paths: fn(&Path) -> Vec<PathBuf>,
    roots: &[PathBuf],
    options: &ScanOptions,
    excludes: &ExcludePolicy,
    output: &mut ScanAccumulator,
) {
    let Some(base) = BaseDirs::new() else {
        output
            .warnings
            .push("home directory is unavailable; global caches were skipped".to_owned());
        return;
    };
    for path in paths(base.home_dir()) {
        if path.is_dir() && !excludes.matches(&path, roots) {
            add_candidate(
                &path,
                category,
                if category == Category::ExpensiveGlobalCache {
                    "large downloaded runtime or model cache"
                } else {
                    "downloaded development-tool cache"
                },
                true,
                options,
                output,
            );
        }
    }
}

/// Returns the exact allowlist for package and tool caches that are cheap to restore.
#[must_use]
pub fn global_cache_paths(home: &Path) -> Vec<PathBuf> {
    let mut paths = [
        ".npm/_cacache",
        ".npm/_npx",
        ".cargo/registry/cache",
        ".cargo/registry/src",
        ".cargo/registry/index",
        ".cargo/git/db",
        "go/pkg/mod",
        ".cache/uv",
        ".cache/pip",
        ".cache/puppeteer",
        ".cache/gem",
        ".pub-cache/hosted",
        ".pub-cache/hosted-hashes",
        ".pub-cache/git",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect::<Vec<_>>();
    if cfg!(target_os = "macos") {
        paths.extend(
            [
                "Library/pnpm",
                "Library/Caches/ms-playwright",
                "Library/Caches/node-gyp",
            ]
            .into_iter()
            .map(|relative| home.join(relative)),
        );
    }
    if cfg!(target_os = "windows") {
        paths.extend(
            [
                "AppData/Local/npm-cache",
                "AppData/Local/pnpm/store",
                "AppData/Local/ms-playwright",
                "AppData/Local/node-gyp/Cache",
            ]
            .into_iter()
            .map(|relative| home.join(relative)),
        );
    }
    paths
}

/// Returns the exact allowlist for caches that can be expensive to download again.
#[must_use]
pub fn expensive_global_cache_paths(home: &Path) -> Vec<PathBuf> {
    [
        ".cache/codex-runtimes",
        ".cache/huggingface",
        ".cache/whisper",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect()
}

#[derive(Debug, Clone, Copy)]
struct ArtifactStats {
    bytes: u64,
    modified: Option<SystemTime>,
}

fn artifact_stats(path: &Path) -> Result<ArtifactStats> {
    let mut bytes = 0_u64;
    let mut modified = None;
    #[cfg(unix)]
    let mut seen = HashSet::new();

    for entry in WalkDir::new(path)
        .follow_links(false)
        .same_file_system(true)
    {
        let entry = entry.with_context(|| format!("failed to walk {}", path.display()))?;
        let metadata = fs::symlink_metadata(entry.path())
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if let Ok(timestamp) = metadata.modified() {
            if modified.is_none_or(|current| timestamp > current) {
                modified = Some(timestamp);
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if !seen.insert((metadata.dev(), metadata.ino())) {
                continue;
            }
            bytes = bytes.saturating_add(metadata.blocks().saturating_mul(512));
        }
        #[cfg(not(unix))]
        {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(ArtifactStats { bytes, modified })
}

fn is_old_enough(modified: Option<SystemTime>, minimum_age: Option<Duration>) -> bool {
    let Some(minimum_age) = minimum_age else {
        return true;
    };
    modified
        .and_then(|timestamp| SystemTime::now().duration_since(timestamp).ok())
        .is_some_and(|age| age >= minimum_age)
}

fn system_time_to_unix(value: SystemTime) -> Option<u64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|age| age.as_secs())
}

/// Groups candidate bytes by category.
#[must_use]
pub fn totals_by_category(report: &ScanReport) -> HashMap<Category, u64> {
    let mut totals = HashMap::new();
    for candidate in &report.candidates {
        *totals.entry(candidate.category).or_default() += candidate.bytes;
    }
    totals
}

#[cfg(test)]
mod tests {
    use std::fs;

    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::*;

    fn options(root: &Path, categories: HashSet<Category>) -> ScanOptions {
        ScanOptions {
            roots: vec![root.to_path_buf()],
            categories,
            include_global_caches: false,
            include_expensive_caches: false,
            max_depth: 16,
            excludes: Vec::new(),
            older_than: None,
            min_size: 0,
            protect_git_tracked: false,
            learning_mode: LearningMode::Disabled,
            approved_review_paths: HashSet::new(),
        }
    }

    #[test]
    fn classify_should_accept_rust_target_with_marker() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target");
        fs::create_dir_all(target.join("debug"))?;

        assert!(matches!(classify(&target), Some((Category::RustTarget, _))));
        Ok(())
    }

    #[test]
    fn classify_should_reject_unmarked_target_directory() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target");
        fs::create_dir_all(&target)?;

        assert!(classify(&target).is_none());
        Ok(())
    }

    #[test]
    fn classify_should_protect_backup_names_case_insensitively() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("Backups/project/node_modules");
        fs::create_dir_all(&modules)?;

        assert!(classify(&modules).is_none());
        Ok(())
    }

    #[test]
    fn scan_should_not_descend_into_node_modules() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(modules.join("nested/node_modules"))?;
        fs::write(modules.join("package.js"), "content")?;

        let report = scan(&options(
            temporary.path(),
            HashSet::from([Category::NodeModules]),
        ))?;

        assert_eq!(report.candidates.len(), 1);
        Ok(())
    }

    #[test]
    fn scan_should_apply_exclude_glob() -> Result<()> {
        let temporary = tempdir()?;
        fs::create_dir_all(temporary.path().join("vendor/node_modules"))?;
        let mut scan_options = options(temporary.path(), HashSet::from([Category::NodeModules]));
        scan_options.excludes.push("vendor/**".to_owned());

        let report = scan(&scan_options)?;

        assert!(report.candidates.is_empty());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn scan_should_not_follow_symlink_to_node_modules() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temporary = tempdir()?;
        let outside = tempdir()?;
        let modules = outside.path().join("node_modules");
        fs::create_dir_all(&modules)?;
        symlink(&modules, temporary.path().join("node_modules"))?;

        let report = scan(&options(
            temporary.path(),
            HashSet::from([Category::NodeModules]),
        ))?;

        assert!(report.candidates.is_empty());
        Ok(())
    }

    #[test]
    fn learning_mode_should_observe_unknown_project_cache_as_review_only() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("package.json"), "{}")?;
        let cache = temporary.path().join("dist");
        fs::create_dir_all(&cache)?;
        fs::write(cache.join("bundle.js"), "generated")?;
        let mut options = options(temporary.path(), HashSet::from(Category::all()));
        options.learning_mode = LearningMode::Enabled;

        let report = scan(&options)?;

        assert!(report.candidates.is_empty());
        assert_eq!(report.review_candidates.len(), 1);
        Ok(())
    }

    #[test]
    fn learning_mode_should_measure_active_artifact_without_making_it_cleanable() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target/debug");
        fs::create_dir_all(&target)?;
        fs::write(target.join("artifact"), "fresh")?;
        let mut options = options(temporary.path(), HashSet::from([Category::RustTarget]));
        options.learning_mode = LearningMode::Enabled;
        options.older_than = Some(Duration::from_secs(86_400));

        let report = scan(&options)?;

        assert!(report.candidates.is_empty());
        assert_eq!(report.learning_observations.len(), 1);
        Ok(())
    }

    #[test]
    fn learning_mode_should_suggest_swift_package_rule_for_dot_build() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("Package.swift"),
            "// swift-tools-version: 6.0",
        )?;
        fs::create_dir_all(temporary.path().join(".build"))?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert_eq!(
            report.review_candidates[0].suggested_rule,
            Some(ReviewRule::SwiftPackageBuild)
        );
        Ok(())
    }

    #[test]
    fn approved_swift_package_rule_should_promote_dot_build_to_candidate() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("Package.swift"),
            "// swift-tools-version: 6.0",
        )?;
        let build = temporary.path().join(".build");
        fs::create_dir_all(&build)?;
        fs::write(build.join("artifact"), "generated")?;
        let mut scan_options = options(temporary.path(), HashSet::new());
        scan_options.learning_mode = LearningMode::Enabled;
        scan_options.approved_review_paths.insert(build);

        let report = scan(&scan_options)?;

        assert_eq!(
            report.candidates[0].approved_rule,
            Some(ReviewRule::SwiftPackageBuild)
        );
        Ok(())
    }

    #[test]
    fn recent_approved_swift_package_build_should_wait_for_age_threshold() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("Package.swift"),
            "// swift-tools-version: 6.0",
        )?;
        let build = temporary.path().join(".build");
        fs::create_dir_all(&build)?;
        let mut scan_options = options(temporary.path(), HashSet::new());
        scan_options.learning_mode = LearningMode::Enabled;
        scan_options.older_than = Some(Duration::from_secs(86_400));
        scan_options.approved_review_paths.insert(build);

        let report = scan(&scan_options)?;

        assert!(report.candidates.is_empty() && report.review_candidates[0].approved);
        Ok(())
    }

    #[test]
    fn approval_should_not_promote_dot_build_without_direct_package_manifest() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("Package.swift"),
            "// swift-tools-version: 6.0",
        )?;
        let nested = temporary.path().join("nested");
        let build = nested.join(".build");
        fs::create_dir_all(&build)?;
        let mut scan_options = options(temporary.path(), HashSet::new());
        scan_options.learning_mode = LearningMode::Enabled;
        scan_options.approved_review_paths.insert(build);

        let report = scan(&scan_options)?;

        assert!(report.candidates.is_empty() && !report.review_candidates[0].approved);
        Ok(())
    }

    proptest! {
        #[test]
        fn protected_names_are_case_insensitive(uppercase in any::<bool>()) {
            let name = if uppercase { "POSTGRES" } else { "postgres" };
            let path = PathBuf::from("root").join(name).join("node_modules");
            prop_assert!(is_protected(&path));
        }
    }
}
