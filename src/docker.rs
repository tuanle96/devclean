use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Returns Docker's detailed, read-only disk usage summary.
///
/// # Errors
///
/// Returns an error when Docker is missing, unavailable, or exits unsuccessfully.
pub fn system_df() -> Result<String> {
    run_docker(&["system", "df", "-v"])
}

/// Returns Docker disk-usage rows as structured JSON values.
///
/// # Errors
///
/// Returns an error when Docker is unavailable or emits an invalid JSON row.
pub fn system_df_structured() -> Result<Vec<Value>> {
    let output = run_docker(&["system", "df", "--format", "{{json .}}"])?;
    parse_system_df(&output)
}

fn parse_system_df(output: &str) -> Result<Vec<Value>> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("Docker emitted an invalid JSON row"))
        .collect()
}

/// Removes only unused Docker build cache.
///
/// # Errors
///
/// Returns an error when Docker is missing, unavailable, or exits unsuccessfully.
pub fn prune_build_cache(older_than: Option<&str>) -> Result<String> {
    let mut arguments = vec!["builder", "prune", "-af"];
    let filter;
    if let Some(age) = older_than {
        filter = format!("until={age}");
        arguments.extend(["--filter", &filter]);
    }
    run_docker(&arguments)
}

/// Removes stopped containers, unused networks/images, and build cache.
///
/// Docker volumes are intentionally never passed to prune.
///
/// # Errors
///
/// Returns an error when Docker is missing, unavailable, or exits unsuccessfully.
pub fn prune_system(older_than: Option<&str>) -> Result<String> {
    let mut arguments = vec!["system", "prune", "-af"];
    let filter;
    if let Some(age) = older_than {
        filter = format!("until={age}");
        arguments.extend(["--filter", &filter]);
    }
    run_docker(&arguments)
}

fn run_docker(arguments: &[&str]) -> Result<String> {
    run_docker_with(arguments, |arguments| {
        let output = Command::new("docker")
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .context("failed to run docker")?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    })
}

fn run_docker_with(
    arguments: &[&str],
    runner: impl FnOnce(&[&str]) -> Result<CommandOutput>,
) -> Result<String> {
    let output = runner(arguments)?;
    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("docker failed: {}", stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_runner_should_forward_arguments_and_surface_stderr() {
        let result = run_docker_with(&["builder", "prune", "-af"], |arguments| {
            assert_eq!(arguments, ["builder", "prune", "-af"]);
            Ok(CommandOutput {
                success: false,
                stdout: Vec::new(),
                stderr: b"daemon unavailable".to_vec(),
            })
        });

        assert!(
            result
                .expect_err("failure status must be rejected")
                .to_string()
                .contains("daemon unavailable")
        );
    }

    #[test]
    fn structured_rows_should_parse_json_lines() -> Result<()> {
        let output =
            "{\"Type\":\"Images\",\"Size\":\"1GB\"}\n{\"Type\":\"Build Cache\",\"Size\":\"2GB\"}\n";
        let rows = parse_system_df(output)?;

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["Type"], "Build Cache");
        Ok(())
    }
}
