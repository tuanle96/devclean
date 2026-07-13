//! Workspace and monorepo detection for grouped scan results.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Candidate, Category};

/// A recognized workspace or monorepo technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceKind {
    /// A Cargo manifest containing a `[workspace]` table.
    Cargo,
    /// A package manifest containing an npm-compatible `workspaces` declaration.
    Npm,
    /// An Nx workspace identified by `nx.json`.
    Nx,
}

impl fmt::Display for WorkspaceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cargo => "cargo",
            Self::Npm => "npm",
            Self::Nx => "nx",
        })
    }
}

/// Aggregate rebuildable artifacts belonging to one workspace root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    /// Workspace root containing the recognized manifest.
    pub root: PathBuf,
    /// Technologies detected at this root.
    pub kinds: Vec<WorkspaceKind>,
    /// Number of cleanup candidates grouped under the workspace.
    pub candidate_count: usize,
    /// Total allocated bytes represented by the grouped candidates.
    pub total_bytes: u64,
    /// Allocated bytes grouped by artifact category.
    pub categories: BTreeMap<Category, u64>,
}

#[derive(Debug, Default)]
struct WorkspaceAccumulator {
    kinds: BTreeSet<WorkspaceKind>,
    candidate_count: usize,
    total_bytes: u64,
    categories: BTreeMap<Category, u64>,
}

/// Groups candidates under the nearest recognized Cargo, npm, or Nx workspace root.
#[must_use]
pub fn summarize(candidates: &[Candidate]) -> Vec<WorkspaceSummary> {
    let mut grouped = BTreeMap::<PathBuf, WorkspaceAccumulator>::new();
    let mut marker_cache = BTreeMap::<PathBuf, Vec<WorkspaceKind>>::new();
    for candidate in candidates {
        let Some((root, kinds)) = detect(&candidate.path, &mut marker_cache) else {
            continue;
        };
        let workspace = grouped.entry(root).or_default();
        workspace.kinds.extend(kinds);
        workspace.candidate_count = workspace.candidate_count.saturating_add(1);
        workspace.total_bytes = workspace.total_bytes.saturating_add(candidate.bytes);
        let category_bytes = workspace.categories.entry(candidate.category).or_default();
        *category_bytes = category_bytes.saturating_add(candidate.bytes);
    }

    let mut summaries = grouped
        .into_iter()
        .map(|(root, workspace)| WorkspaceSummary {
            root,
            kinds: workspace.kinds.into_iter().collect(),
            candidate_count: workspace.candidate_count,
            total_bytes: workspace.total_bytes,
            categories: workspace.categories,
        })
        .collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        right
            .total_bytes
            .cmp(&left.total_bytes)
            .then_with(|| left.root.cmp(&right.root))
    });
    summaries
}

fn detect(
    candidate: &Path,
    marker_cache: &mut BTreeMap<PathBuf, Vec<WorkspaceKind>>,
) -> Option<(PathBuf, Vec<WorkspaceKind>)> {
    for directory in candidate.parent()?.ancestors() {
        let kinds = marker_cache
            .entry(directory.to_path_buf())
            .or_insert_with(|| kinds_at(directory));
        if !kinds.is_empty() {
            return Some((directory.to_path_buf(), kinds.clone()));
        }
    }
    None
}

fn kinds_at(directory: &Path) -> Vec<WorkspaceKind> {
    let mut kinds = Vec::new();
    if is_cargo_workspace(directory) {
        kinds.push(WorkspaceKind::Cargo);
    }
    if is_npm_workspace(directory) {
        kinds.push(WorkspaceKind::Npm);
    }
    if directory.join("nx.json").is_file() {
        kinds.push(WorkspaceKind::Nx);
    }
    kinds
}

fn is_cargo_workspace(directory: &Path) -> bool {
    fs::read_to_string(directory.join("Cargo.toml"))
        .ok()
        .and_then(|source| source.parse::<toml::Value>().ok())
        .is_some_and(|manifest| manifest.get("workspace").is_some())
}

fn is_npm_workspace(directory: &Path) -> bool {
    fs::read_to_string(directory.join("package.json"))
        .ok()
        .and_then(|source| serde_json::from_str::<serde_json::Value>(&source).ok())
        .and_then(|manifest| manifest.get("workspaces").cloned())
        .is_some_and(|workspaces| workspaces.is_array() || workspaces.is_object())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use tempfile::tempdir;

    use super::*;
    use crate::model::Confidence;

    fn candidate(path: PathBuf, category: Category, bytes: u64) -> Candidate {
        Candidate {
            category,
            path,
            bytes,
            reason: "test".to_owned(),
            modified_at_unix: None,
            confidence: Confidence::Safe,
            approved_rule: None,
            custom_rule: None,
        }
    }

    #[test]
    fn cargo_member_target_should_group_under_workspace_root() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )?;
        let target = temporary.path().join("member/target");
        fs::create_dir_all(&target)?;

        let summaries = summarize(&[candidate(target, Category::RustTarget, 4096)]);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].root, temporary.path());
        assert_eq!(summaries[0].kinds, vec![WorkspaceKind::Cargo]);
        assert_eq!(summaries[0].total_bytes, 4096);
        Ok(())
    }

    #[test]
    fn npm_and_nx_markers_should_share_one_workspace_summary() -> Result<()> {
        let temporary = tempdir()?;
        fs::write(
            temporary.path().join("package.json"),
            r#"{"workspaces":["packages/*"]}"#,
        )?;
        fs::write(temporary.path().join("nx.json"), "{}")?;
        let modules = temporary.path().join("packages/app/node_modules");
        fs::create_dir_all(&modules)?;

        let summaries = summarize(&[candidate(modules, Category::NodeModules, 2048)]);

        assert_eq!(
            summaries[0].kinds,
            vec![WorkspaceKind::Npm, WorkspaceKind::Nx]
        );
        assert_eq!(summaries[0].candidate_count, 1);
        Ok(())
    }

    #[test]
    fn standalone_project_should_not_be_reported_as_workspace() -> Result<()> {
        let temporary = tempdir()?;
        let modules = temporary.path().join("node_modules");
        fs::create_dir_all(&modules)?;

        assert!(summarize(&[candidate(modules, Category::NodeModules, 1)]).is_empty());
        Ok(())
    }
}
