use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::model::{Candidate, Category, ScanReport};

/// Scanner configuration.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Filesystem roots to inspect.
    pub roots: Vec<PathBuf>,
    /// Artifact categories to include.
    pub categories: HashSet<Category>,
    /// Include known global package and tool caches.
    pub include_global_caches: bool,
    /// Maximum traversal depth below each root.
    pub max_depth: usize,
}

/// Returns useful development roots without scanning the entire home directory.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        for relative in ["Dev", "Documents/Codex"] {
            let candidate = home.join(relative);
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
/// Returns an error when no valid root exists or a root cannot be normalized.
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
        normalized_roots.push(normalized.clone());
        scan_root(&normalized, options, &mut candidates, &mut warnings);
    }

    if normalized_roots.is_empty() {
        anyhow::bail!("no valid scan roots were found");
    }

    if options.include_global_caches && options.categories.contains(&Category::GlobalCache) {
        candidates.extend(global_cache_candidates(&mut warnings));
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
    })
}

fn scan_root(
    root: &Path,
    options: &ScanOptions,
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
        if path != root && should_prune(path) {
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

        match allocated_size(path) {
            Ok(bytes) => candidates.push(Candidate {
                category,
                path: path.to_path_buf(),
                bytes,
                reason: reason.to_owned(),
            }),
            Err(error) => warnings.push(format!("{}: {error:#}", path.display())),
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
        matches_name(
            name,
            &[".git", ".hg", ".svn", "backups", "backup", "volumes"],
        ) || name == OsStr::new("postgres")
    })
}

fn matches_name(name: &OsStr, values: &[&str]) -> bool {
    values.iter().any(|value| name == OsStr::new(value))
}

fn global_cache_candidates(warnings: &mut Vec<String>) -> Vec<Candidate> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        warnings.push("HOME is not set; global caches were skipped".to_owned());
        return Vec::new();
    };
    global_cache_paths(&home)
        .into_iter()
        .filter(|path| path.is_dir())
        .filter_map(|path| match allocated_size(&path) {
            Ok(bytes) => Some(Candidate {
                category: Category::GlobalCache,
                path,
                bytes,
                reason: "downloaded development-tool cache".to_owned(),
            }),
            Err(error) => {
                warnings.push(error.to_string());
                None
            }
        })
        .collect()
}

/// Returns the exact global-cache allowlist for a home directory.
#[must_use]
pub fn global_cache_paths(home: &Path) -> Vec<PathBuf> {
    [
        ".npm/_cacache",
        ".npm/_npx",
        ".cargo/registry/cache",
        ".cargo/registry/src",
        ".cargo/registry/index",
        ".cargo/git/db",
        "Library/pnpm",
        "go/pkg/mod",
        ".cache/uv",
        ".cache/pip",
        ".pub-cache/hosted",
        ".pub-cache/hosted-hashes",
        ".pub-cache/git",
        ".cache/codex-runtimes",
        ".cache/puppeteer",
        ".cache/huggingface",
        ".cache/whisper",
        ".cache/gem",
        "Library/Caches/ms-playwright",
        "Library/Caches/node-gyp",
    ]
    .into_iter()
    .map(|relative| home.join(relative))
    .collect()
}

fn allocated_size(path: &Path) -> Result<u64> {
    let mut bytes = 0_u64;
    #[cfg(unix)]
    let mut seen = HashSet::new();

    for entry in WalkDir::new(path)
        .follow_links(false)
        .same_file_system(true)
    {
        let entry = entry.with_context(|| format!("failed to walk {}", path.display()))?;
        let metadata = fs::symlink_metadata(entry.path())
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;

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
    Ok(bytes)
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

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn classify_should_accept_rust_target_with_marker() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target");
        fs::create_dir_all(target.join("debug"))?;

        let result = classify(&target);

        assert!(matches!(result, Some((Category::RustTarget, _))));
        Ok(())
    }

    #[test]
    fn classify_should_reject_unmarked_target_directory() -> Result<()> {
        let temporary = tempdir()?;
        let target = temporary.path().join("target");
        fs::create_dir_all(&target)?;

        let result = classify(&target);

        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn scan_should_not_descend_into_node_modules() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(modules.join("nested/node_modules"))?;
        fs::write(modules.join("package.js"), "content")?;
        let options = ScanOptions {
            roots: vec![temporary.path().to_path_buf()],
            categories: HashSet::from([Category::NodeModules]),
            include_global_caches: false,
            max_depth: 16,
        };

        let report = scan(&options)?;

        assert_eq!(report.candidates.len(), 1);
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
        let options = ScanOptions {
            roots: vec![temporary.path().to_path_buf()],
            categories: HashSet::from([Category::NodeModules]),
            include_global_caches: false,
            max_depth: 16,
        };

        let report = scan(&options)?;

        assert!(report.candidates.is_empty());
        Ok(())
    }
}
