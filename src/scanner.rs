use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use directories::BaseDirs;
use walkdir::WalkDir;

use crate::model::{
    Candidate, Category, Confidence, CustomRule, LearningObservation, ReviewCandidate, ReviewRule,
    ScanReport,
};
use crate::policy::{ExcludePolicy, GitTrackedGuard};
use crate::workspace;

mod classifier;
mod global_caches;
mod measurement;

#[cfg(test)]
use classifier::is_home_directory;
pub use classifier::{classify, classify_approved_review_candidate, matches_custom_rule};
use classifier::{classify_review_candidate, should_prune};
pub use global_caches::{expensive_global_cache_paths, global_cache_paths};
use measurement::{ArtifactStats, measure_pending_artifacts};

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
    /// Exact-name, direct-marker rules loaded from configuration.
    pub custom_rules: Vec<CustomRule>,
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
    git_guard: GitTrackedGuard,
}

#[derive(Debug)]
enum PendingKind {
    Candidate(CandidateClassification),
    Review { reason: &'static str },
}

#[derive(Debug)]
struct CandidateClassification {
    category: Category,
    reason: String,
    include_cleanup_candidate: bool,
    custom_rule: Option<CustomRule>,
}

/// A classified directory awaiting size and modification-time measurement.
#[derive(Debug)]
struct PendingArtifact {
    path: PathBuf,
    kind: PendingKind,
}

/// Returns useful development roots without scanning the entire home directory.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(base) = BaseDirs::new() {
        for relative in ["Dev", "Projects"] {
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
    let mut pending = Vec::new();
    for root in &normalized_roots {
        scan_root(
            root,
            &normalized_roots,
            &effective_options,
            &excludes,
            &mut pending,
            &mut output.warnings,
        );
    }

    if effective_options.include_global_caches
        && effective_options
            .categories
            .contains(&Category::GlobalCache)
    {
        collect_global_cache_candidates(
            Category::GlobalCache,
            global_cache_paths,
            &normalized_roots,
            &excludes,
            &mut pending,
            &mut output.warnings,
        );
    }
    if effective_options.include_expensive_caches
        && effective_options
            .categories
            .contains(&Category::ExpensiveGlobalCache)
    {
        collect_global_cache_candidates(
            Category::ExpensiveGlobalCache,
            expensive_global_cache_paths,
            &normalized_roots,
            &excludes,
            &mut pending,
            &mut output.warnings,
        );
    }

    process_pending_artifacts(pending, &effective_options, &mut output);

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
    let workspaces = workspace::summarize(&output.candidates);

    Ok(ScanReport {
        roots: normalized_roots,
        candidates: output.candidates,
        review_candidates: output.review_candidates,
        learning_observations: output.learning_observations,
        workspaces,
        warnings: output.warnings,
        total_bytes,
        review_total_bytes,
        observed_total_bytes,
        protect_git_tracked: effective_options.protect_git_tracked,
    })
}

fn process_pending_artifacts(
    pending: Vec<PendingArtifact>,
    options: &ScanOptions,
    output: &mut ScanAccumulator,
) {
    let stats = measure_pending_artifacts(&pending);
    for (artifact, stats) in pending.into_iter().zip(stats) {
        match artifact.kind {
            PendingKind::Candidate(classification) => {
                add_candidate(&artifact.path, stats, classification, options, output);
            }
            PendingKind::Review { reason } => {
                add_review_candidate(&artifact.path, stats, reason, options, output);
            }
        }
    }
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
    pending: &mut Vec<PendingArtifact>,
    warnings: &mut Vec<String>,
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
                warnings.push(error.to_string());
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

        let classified = classify(path)
            .map(|(category, reason)| (category, reason.to_owned(), None))
            .or_else(|| {
                options.custom_rules.iter().find_map(|rule| {
                    matches_custom_rule(path, rule)
                        .then(|| (rule.category, rule.reason.clone(), Some(rule.clone())))
                })
            });
        let Some((category, reason, custom_rule)) = classified else {
            if options.learning_mode.is_enabled() || options.approved_review_paths.contains(path) {
                if let Some(reason) = classify_review_candidate(path) {
                    walker.skip_current_dir();
                    pending.push(PendingArtifact {
                        path: path.to_path_buf(),
                        kind: PendingKind::Review { reason },
                    });
                }
            }
            continue;
        };
        walker.skip_current_dir();
        pending.push(PendingArtifact {
            path: path.to_path_buf(),
            kind: PendingKind::Candidate(CandidateClassification {
                category,
                reason,
                include_cleanup_candidate: options.categories.contains(&category),
                custom_rule,
            }),
        });
    }
}

fn add_candidate(
    path: &Path,
    stats: Result<ArtifactStats>,
    classification: CandidateClassification,
    options: &ScanOptions,
    output: &mut ScanAccumulator,
) {
    let CandidateClassification {
        category,
        reason,
        include_cleanup_candidate,
        custom_rule,
    } = classification;
    let stats = match stats {
        Ok(stats) => stats,
        Err(error) => {
            output
                .warnings
                .push(format!("{}: {error:#}", path.display()));
            return;
        }
    };
    if options.protect_git_tracked {
        match output.git_guard.contains_tracked_files(path) {
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
            reason: reason.clone(),
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
        reason,
        modified_at_unix: stats.modified.and_then(system_time_to_unix),
        confidence: Confidence::Safe,
        approved_rule: None,
        custom_rule,
    });
}

fn add_review_candidate(
    path: &Path,
    stats: Result<ArtifactStats>,
    reason: &'static str,
    options: &ScanOptions,
    output: &mut ScanAccumulator,
) {
    let stats = match stats {
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
        match output.git_guard.contains_tracked_files(path) {
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
                custom_rule: None,
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
    ReviewRule::all().into_iter().find_map(|rule| {
        classify_approved_review_candidate(path, rule).map(|_| (rule, parent.to_path_buf()))
    })
}

fn collect_global_cache_candidates(
    category: Category,
    paths: fn(&Path) -> Vec<PathBuf>,
    roots: &[PathBuf],
    excludes: &ExcludePolicy,
    pending: &mut Vec<PendingArtifact>,
    warnings: &mut Vec<String>,
) {
    let Some(base) = BaseDirs::new() else {
        warnings.push("home directory is unavailable; global caches were skipped".to_owned());
        return;
    };
    for path in paths(base.home_dir()) {
        if path.is_dir() && !excludes.matches(&path, roots) {
            pending.push(PendingArtifact {
                path,
                kind: PendingKind::Candidate(CandidateClassification {
                    category,
                    reason: if category == Category::ExpensiveGlobalCache {
                        "large downloaded runtime or model cache".to_owned()
                    } else {
                        "downloaded development-tool cache".to_owned()
                    },
                    include_cleanup_candidate: true,
                    custom_rule: None,
                }),
            });
        }
    }
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
pub fn totals_by_category(report: &ScanReport) -> BTreeMap<Category, u64> {
    let mut totals = BTreeMap::new();
    for candidate in &report.candidates {
        *totals.entry(candidate.category).or_default() += candidate.bytes;
    }
    totals
}

#[cfg(test)]
mod tests {
    use std::fs;

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
            custom_rules: Vec::new(),
        }
    }

    #[test]
    fn scan_should_find_python_bytecode_and_test_environments() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("pyproject.toml"),
            "[project]\nname='demo'\n",
        )?;
        fs::create_dir_all(temporary.path().join("src/__pycache__"))?;
        fs::create_dir_all(temporary.path().join(".tox/py311"))?;
        let report = scan(&options(
            temporary.path(),
            HashSet::from([Category::PythonCache]),
        ))?;

        assert_eq!(report.candidates.len(), 2);
        assert!(
            report
                .candidates
                .iter()
                .all(|candidate| candidate.category == Category::PythonCache)
        );
        Ok(())
    }

    #[test]
    fn custom_rule_should_require_exact_name_and_every_direct_marker() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("project.lock"), "locked")?;
        fs::write(temporary.path().join("project.toml"), "configured")?;
        let generated = temporary.path().join("generated-cache");
        fs::create_dir_all(&generated)?;
        let rule = CustomRule {
            name: "project-generated-cache".to_owned(),
            category: Category::BuildOutput,
            directory_names: vec!["generated-cache".to_owned()],
            required_markers: vec!["project.toml".to_owned(), "project.lock".to_owned()],
            reason: "team-approved generated cache".to_owned(),
        };
        let mut scan_options = options(temporary.path(), HashSet::from([Category::BuildOutput]));
        scan_options.custom_rules.push(rule.clone());

        let report = scan(&scan_options)?;

        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].custom_rule, Some(rule.clone()));
        fs::remove_file(temporary.path().join("project.lock"))?;
        assert!(!matches_custom_rule(&generated, &rule));
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

    #[test]
    fn learning_mode_should_suggest_xcode_rule_for_derived_data() -> Result<()> {
        let temporary = tempdir()?;
        fs::create_dir_all(temporary.path().join("App.xcodeproj"))?;
        let derived = temporary.path().join("DerivedData");
        fs::create_dir_all(&derived)?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert_eq!(
            report.review_candidates[0].suggested_rule,
            Some(ReviewRule::XcodeDerivedData)
        );
        Ok(())
    }

    #[test]
    fn learning_mode_should_suggest_gradle_rule_for_kotlin_dsl_project() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("build.gradle.kts"), "plugins {}")?;
        fs::create_dir_all(temporary.path().join(".gradle"))?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert_eq!(
            report.review_candidates[0].suggested_rule,
            Some(ReviewRule::GradleBuild)
        );
        Ok(())
    }

    #[test]
    fn learning_mode_should_suggest_cocoapods_rule_with_lockfile() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("Podfile"), "platform :ios")?;
        fs::write(temporary.path().join("Podfile.lock"), "PODS:\n")?;
        fs::create_dir_all(temporary.path().join("Pods"))?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert_eq!(
            report.review_candidates[0].suggested_rule,
            Some(ReviewRule::CocoaPods)
        );
        Ok(())
    }

    #[test]
    fn cocoapods_rule_should_reject_pods_without_lockfile() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("Podfile"), "platform :ios")?;
        let pods = temporary.path().join("Pods");
        fs::create_dir_all(&pods)?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert_eq!(report.review_candidates.len(), 1);
        assert!(report.review_candidates[0].suggested_rule.is_none());
        assert!(
            classify_approved_review_candidate(&pods, ReviewRule::CocoaPods).is_none(),
            "a Podfile alone is not enough evidence for reproducible cleanup"
        );
        Ok(())
    }

    #[test]
    fn approved_cocoapods_rule_should_promote_locked_pods_to_candidate() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("Podfile"), "platform :ios")?;
        let lockfile = temporary.path().join("Podfile.lock");
        fs::write(&lockfile, "PODS:\n")?;
        let pods = temporary.path().join("Pods");
        fs::create_dir_all(&pods)?;
        fs::write(pods.join("dependency.m"), "generated")?;
        let mut scan_options = options(temporary.path(), HashSet::new());
        scan_options.learning_mode = LearningMode::Enabled;
        scan_options.approved_review_paths.insert(pods.clone());

        let report = scan(&scan_options)?;

        assert_eq!(
            report.candidates[0].approved_rule,
            Some(ReviewRule::CocoaPods)
        );
        fs::remove_file(lockfile)?;
        assert!(classify_approved_review_candidate(&pods, ReviewRule::CocoaPods).is_none());
        Ok(())
    }

    #[test]
    fn approved_gradle_rule_should_promote_cache_to_candidate() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("settings.gradle"), "rootProject")?;
        let cache = temporary.path().join(".gradle");
        fs::create_dir_all(&cache)?;
        fs::write(cache.join("checksums.bin"), "generated")?;
        let mut scan_options = options(temporary.path(), HashSet::new());
        scan_options.learning_mode = LearningMode::Enabled;
        scan_options.approved_review_paths.insert(cache);

        let report = scan(&scan_options)?;

        assert_eq!(
            report.candidates[0].approved_rule,
            Some(ReviewRule::GradleBuild)
        );
        Ok(())
    }

    #[test]
    fn gradle_rule_should_reject_the_global_gradle_directory_in_home() -> Result<()> {
        let temporary = tempdir()?;
        assert!(!is_home_directory(temporary.path()));
        let home = BaseDirs::new().map(|base| base.home_dir().to_path_buf());
        if let Some(home) = home {
            assert!(is_home_directory(&home));
            assert!(
                classify_approved_review_candidate(&home.join(".gradle"), ReviewRule::GradleBuild)
                    .is_none()
            );
        }
        Ok(())
    }

    #[test]
    fn derived_data_without_xcode_container_should_stay_unsuggested() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(temporary.path().join("package.json"), "{}")?;
        fs::create_dir_all(temporary.path().join("DerivedData"))?;
        let mut scan_options = options(temporary.path(), HashSet::from(Category::all()));
        scan_options.learning_mode = LearningMode::Enabled;

        let report = scan(&scan_options)?;

        assert!(
            report.review_candidates[0].suggested_rule.is_none()
                && !report.review_candidates[0].approved
        );
        Ok(())
    }
}
