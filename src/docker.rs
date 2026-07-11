use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Returns Docker's read-only disk usage summary.
///
/// # Errors
///
/// Returns an error when Docker is missing, unavailable, or exits unsuccessfully.
pub fn system_df() -> Result<String> {
    run_docker(&["system", "df"])
}

/// Removes stopped containers, unused networks/images, and build cache.
///
/// Docker volumes are intentionally never passed to prune.
///
/// # Errors
///
/// Returns an error when Docker is missing, unavailable, or exits unsuccessfully.
pub fn prune() -> Result<String> {
    run_docker(&["system", "prune", "-af"])
}

fn run_docker(arguments: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .context("failed to run docker")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("docker failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
