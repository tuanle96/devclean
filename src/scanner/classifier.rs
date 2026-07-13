use std::ffi::OsStr;
use std::path::{Component, Path};

use directories::BaseDirs;

use crate::model::{Category, CustomRule, ReviewRule};

/// Revalidates a user-approved review path against its scanner-owned rule.
#[must_use]
pub fn classify_approved_review_candidate(
    path: &Path,
    rule: ReviewRule,
) -> Option<(Category, &'static str)> {
    if is_protected(path) {
        return None;
    }
    let parent = path.parent()?;
    match rule {
        ReviewRule::SwiftPackageBuild => (path.file_name() == Some(OsStr::new(".build"))
            && parent.join("Package.swift").is_file())
        .then_some((
            Category::BuildOutput,
            "user-approved Swift Package build directory",
        )),
        ReviewRule::XcodeDerivedData => (path.file_name() == Some(OsStr::new("DerivedData"))
            && has_xcode_container(parent))
        .then_some((
            Category::BuildOutput,
            "user-approved Xcode DerivedData directory",
        )),
        ReviewRule::GradleBuild => (path.file_name() == Some(OsStr::new(".gradle"))
            && !is_home_directory(parent)
            && [
                "build.gradle",
                "build.gradle.kts",
                "settings.gradle",
                "settings.gradle.kts",
            ]
            .iter()
            .any(|marker| parent.join(marker).is_file()))
        .then_some((
            Category::BuildOutput,
            "user-approved Gradle project cache directory",
        )),
        ReviewRule::CocoaPods => (path.file_name() == Some(OsStr::new("Pods"))
            && parent.join("Podfile").is_file()
            && parent.join("Podfile.lock").is_file())
        .then_some((
            Category::BuildOutput,
            "user-approved CocoaPods dependency directory",
        )),
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
    if matches_name(name, &[".zig-cache", "zig-cache", "zig-out"])
        && path
            .parent()
            .is_some_and(|parent| parent.join("build.zig").is_file())
    {
        return Some((Category::BuildOutput, "Zig compiler output"));
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
    if name == OsStr::new("__pycache__") {
        return Some((Category::PythonCache, "regenerable Python bytecode cache"));
    }
    if matches_name(name, &[".tox", ".nox"]) && path.parent().is_some_and(has_python_project_marker)
    {
        return Some((
            Category::PythonCache,
            "project-local Python test environment cache",
        ));
    }
    if matches_name(name, &[".venv", "venv"])
        && path.parent().is_some_and(has_python_project_marker)
    {
        return Some((
            Category::PythonEnvironment,
            "project-local Python virtual environment with dependency manifest",
        ));
    }
    if name == OsStr::new("build") && looks_like_project_build(path) {
        return Some((
            Category::BuildOutput,
            "build directory beneath a recognized project",
        ));
    }
    None
}

/// Revalidates a config-defined rule using exact candidate names and direct sibling markers.
#[must_use]
pub fn matches_custom_rule(path: &Path, rule: &CustomRule) -> bool {
    if is_protected(path) {
        return false;
    }
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    rule.directory_names.iter().any(|value| value == name)
        && rule
            .required_markers
            .iter()
            .all(|marker| parent.join(marker).is_file())
}

pub(super) fn classify_review_candidate(path: &Path) -> Option<&'static str> {
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
            "build.gradle.kts",
            "settings.gradle.kts",
            "Podfile",
        ]
        .iter()
        .any(|marker| ancestor.join(marker).is_file())
            || has_xcode_container(ancestor)
    })
}

fn has_python_project_marker(path: &Path) -> bool {
    [
        "pyproject.toml",
        "requirements.txt",
        "requirements-dev.txt",
        "Pipfile",
        "Pipfile.lock",
        "poetry.lock",
        "uv.lock",
        "setup.py",
        "setup.cfg",
        "tox.ini",
        "noxfile.py",
    ]
    .iter()
    .any(|marker| path.join(marker).is_file())
}

fn has_xcode_container(path: &Path) -> bool {
    path.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_ok_and(|file_type| file_type.is_dir())
                && matches!(
                    entry.path().extension().and_then(OsStr::to_str),
                    Some("xcodeproj" | "xcworkspace")
                )
        })
}

/// The global `~/.gradle` holds credentials and is never a rebuildable project cache.
pub(super) fn is_home_directory(path: &Path) -> bool {
    BaseDirs::new().is_some_and(|base| {
        let home = base.home_dir();
        path == home || home.canonicalize().is_ok_and(|canonical| path == canonical)
    })
}

pub(super) fn should_prune(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches_name(name, &[".git", ".hg", ".svn", "site-packages"])
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
    if [
        "package.json",
        "pubspec.yaml",
        "Cargo.toml",
        "CMakeLists.txt",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ]
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

pub(super) fn is_protected(path: &Path) -> bool {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::Result;
    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::*;

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
    fn classify_should_accept_python_virtual_environment_with_direct_manifest() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("pyproject.toml"),
            "[project]\nname='demo'\n",
        )?;
        let environment = temporary.path().join(".venv");
        fs::create_dir_all(environment.join("lib/python/site-packages"))?;

        assert!(matches!(
            classify(&environment),
            Some((Category::PythonEnvironment, _))
        ));
        Ok(())
    }

    #[test]
    fn classify_should_reject_unmarked_python_virtual_environment() -> Result<()> {
        let temporary = tempdir()?;
        let environment = temporary.path().join(".venv");
        fs::create_dir_all(&environment)?;

        assert!(classify(&environment).is_none());
        Ok(())
    }

    #[test]
    fn classify_should_accept_gradle_cmake_and_zig_build_outputs() -> Result<()> {
        let temporary = tempdir()?;
        let gradle = temporary.path().join("gradle");
        let cmake = temporary.path().join("cmake");
        let zig = temporary.path().join("zig");
        fs::create_dir_all(gradle.join("build"))?;
        fs::create_dir_all(cmake.join("build"))?;
        fs::create_dir_all(zig.join("zig-out"))?;
        fs::write(gradle.join("build.gradle.kts"), "plugins {}")?;
        fs::write(cmake.join("CMakeLists.txt"), "project(Demo)")?;
        fs::write(zig.join("build.zig"), "const std = @import(\"std\");")?;

        assert!(matches!(
            classify(&gradle.join("build")),
            Some((Category::BuildOutput, _))
        ));
        assert!(matches!(
            classify(&cmake.join("build")),
            Some((Category::BuildOutput, _))
        ));
        assert!(matches!(
            classify(&zig.join("zig-out")),
            Some((Category::BuildOutput, _))
        ));
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
