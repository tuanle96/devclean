use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use directories::BaseDirs;
use walkdir::WalkDir;

use crate::model::{Candidate, Category, ScanReport};
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
    let mut candidates = Vec::new();
    let mut warnings = Vec::new();
    let mut normalized_roots = Vec::new();

    for root in &options.roots {
        if !root.is_dir() {
            warnings.push(format!("skipped missing root: {}", root.display()));
            continue;
        }
        let normalized = root
            .canonicalize()
            .with_context(|| format!("failed to normalize root {}", root.display()))?;
        normalized_roots.push(normalized);
    }
    if normalized_roots.is_empty() {
        anyhow::bail!("no valid scan roots were found");
    }

    let excludes = ExcludePolicy::new(&options.excludes)?;
    for root in &normalized_roots {
        scan_root(
            root,
            &normalized_roots,
            options,
            &excludes,
            &mut candidates,
            &mut warnings,
        );
    }

    if options.include_global_caches && options.categories.contains(&Category::GlobalCache) {
        candidates.extend(global_cache_candidates(
            Category::GlobalCache,
            global_cache_paths,
            &normalized_roots,
            options,
            &excludes,
            &mut warnings,
        ));
    }
    if options.include_expensive_caches
        && options.categories.contains(&Category::ExpensiveGlobalCache)
    {
        candidates.extend(global_cache_candidates(
            Category::ExpensiveGlobalCache,
            expensive_global_cache_paths,
            &normalized_roots,
            options,
            &excludes,
            &mut warnings,
        ));
    }

    candidates.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    let total_bytes = candidates.iter().map(|candidate| candidate.bytes).sum();

    Ok(ScanReport {
        roots: normalized_roots,
        candidates,
        warnings,
        total_bytes,
        protect_git_tracked: options.protect_git_tracked,
    })
}

fn scan_root(
    root: &Path,
    roots: &[PathBuf],
    options: &ScanOptions,
    excludes: &ExcludePolicy,
    candidates: &mut Vec<Candidate>,
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

        let Some((category, reason)) = classify(path) else {
            continue;
        };
        walker.skip_current_dir();
        if !options.categories.contains(&category) {
            continue;
        }
        add_candidate(path, category, reason, options, candidates, warnings);
    }
}

fn add_candidate(
    path: &Path,
    category: Category,
    reason: &'static str,
    options: &ScanOptions,
    candidates: &mut Vec<Candidate>,
    warnings: &mut Vec<String>,
) {
    let stats = match artifact_stats(path) {
        Ok(stats) => stats,
        Err(error) => {
            warnings.push(format!("{}: {error:#}", path.display()));
            return;
        }
    };
    if stats.bytes < options.min_size || !is_old_enough(stats.modified, options.older_than) {
        return;
    }
    if options.protect_git_tracked {
        match contains_git_tracked_files(path) {
            Ok(true) => {
                warnings.push(format!(
                    "protected Git-tracked candidate: {}",
                    path.display()
                ));
                return;
            }
            Ok(false) => {}
            Err(error) => {
                warnings.push(format!(
                    "skipped {} because tracked-file guard failed: {error:#}",
                    path.display()
                ));
                return;
            }
        }
    }
    candidates.push(Candidate {
        category,
        path: path.to_path_buf(),
        bytes: stats.bytes,
        reason: reason.to_owned(),
        modified_at_unix: stats.modified.and_then(system_time_to_unix),
    });
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

fn should_prune(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| matches_name(name, &[".git", ".hg", ".svn", ".venv", "site-packages"]))
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

fn global_cache_candidates(
    category: Category,
    paths: fn(&Path) -> Vec<PathBuf>,
    roots: &[PathBuf],
    options: &ScanOptions,
    excludes: &ExcludePolicy,
    warnings: &mut Vec<String>,
) -> Vec<Candidate> {
    let Some(base) = BaseDirs::new() else {
        warnings.push("home directory is unavailable; global caches were skipped".to_owned());
        return Vec::new();
    };
    let mut candidates = Vec::new();
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
                options,
                &mut candidates,
                warnings,
            );
        }
    }
    candidates
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

    proptest! {
        #[test]
        fn protected_names_are_case_insensitive(uppercase in any::<bool>()) {
            let name = if uppercase { "POSTGRES" } else { "postgres" };
            let path = PathBuf::from("root").join(name).join("node_modules");
            prop_assert!(is_protected(&path));
        }
    }
}
