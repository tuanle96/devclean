use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::{Context, Result};
use rayon::prelude::*;
use walkdir::WalkDir;

use super::PendingArtifact;

#[derive(Debug, Clone, Copy)]
pub(super) struct ArtifactStats {
    pub(super) bytes: u64,
    pub(super) modified: Option<SystemTime>,
}

/// Measures classified directories in parallel while preserving input order.
pub(super) fn measure_pending_artifacts(pending: &[PendingArtifact]) -> Vec<Result<ArtifactStats>> {
    pending
        .par_iter()
        .map(|artifact| artifact_stats(&artifact.path))
        .collect()
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
