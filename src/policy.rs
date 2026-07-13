use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};

/// Compiled path exclusions used during scan and cleanup revalidation.
#[derive(Debug)]
pub struct ExcludePolicy {
    patterns: Vec<String>,
    matcher: GlobSet,
}

/// Reuses repository-root discovery while validating multiple cleanup candidates.
#[derive(Debug, Default)]
pub struct GitTrackedGuard {
    roots_by_directory: HashMap<PathBuf, Option<PathBuf>>,
}

impl GitTrackedGuard {
    /// Returns true when Git tracks the candidate itself or content below it.
    ///
    /// # Errors
    ///
    /// Returns an error when Git cannot inspect an applicable repository.
    pub fn contains_tracked_files(&mut self, path: &Path) -> Result<bool> {
        let probe = path.parent().unwrap_or(path);
        let Some(root) = self.repository_root(probe)? else {
            return Ok(false);
        };
        let relative = path
            .strip_prefix(&root)
            .with_context(|| format!("{} escaped Git root {}", path.display(), root.display()))?;
        let output = Command::new("git")
            .args(["-C"])
            .arg(&root)
            .args(["ls-files", "--"])
            .arg(relative)
            .output()
            .context("failed to list Git-tracked files")?;
        if !output.status.success() {
            bail!("git ls-files failed for {}", path.display());
        }
        Ok(!output.stdout.is_empty())
    }

    fn repository_root(&mut self, probe: &Path) -> Result<Option<PathBuf>> {
        let mut visited = Vec::new();
        for ancestor in probe.ancestors() {
            if let Some(cached) = self.roots_by_directory.get(ancestor).cloned() {
                for directory in visited {
                    self.roots_by_directory.insert(directory, cached.clone());
                }
                return Ok(cached);
            }
            visited.push(ancestor.to_path_buf());
            if ancestor.join(".git").exists() {
                let root = ancestor
                    .canonicalize()
                    .context("failed to normalize Git root")?;
                for directory in visited {
                    self.roots_by_directory
                        .insert(directory, Some(root.clone()));
                }
                return Ok(Some(root));
            }
        }
        for directory in visited {
            self.roots_by_directory.insert(directory, None);
        }
        Ok(None)
    }
}

impl ExcludePolicy {
    /// Compiles user-provided glob patterns.
    ///
    /// # Errors
    ///
    /// Returns an error when a pattern is invalid.
    pub fn new(patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            builder.add(
                Glob::new(pattern)
                    .with_context(|| format!("invalid exclude pattern `{pattern}`"))?,
            );
        }
        Ok(Self {
            patterns: patterns.to_vec(),
            matcher: builder.build()?,
        })
    }

    /// Returns true when an absolute, root-relative, or basename form matches.
    #[must_use]
    pub fn matches(&self, path: &Path, roots: &[PathBuf]) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        if self.matcher.is_match(normalize(path)) {
            return true;
        }
        if path
            .file_name()
            .is_some_and(|name| self.matcher.is_match(name))
        {
            return true;
        }
        roots.iter().any(|root| {
            path.strip_prefix(root)
                .is_ok_and(|relative| self.matcher.is_match(normalize(relative)))
        })
    }
}

/// Returns true when Git tracks the candidate itself or any content below it.
///
/// # Errors
///
/// Returns an error when Git is installed but cannot inspect an applicable repository.
pub fn contains_git_tracked_files(path: &Path) -> Result<bool> {
    GitTrackedGuard::default().contains_tracked_files(path)
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn exclude_policy_should_match_root_relative_glob() -> Result<()> {
        let temporary = tempdir()?;
        let path = temporary.path().join("vendor/node_modules");
        let policy = ExcludePolicy::new(&["vendor/**".to_owned()])?;

        assert!(policy.matches(&path, &[temporary.path().to_path_buf()]));
        Ok(())
    }
}
