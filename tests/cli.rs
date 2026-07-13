use std::fs;
use std::process::Command;

use anyhow::Result;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn scan_should_emit_redacted_json() -> Result<()> {
    let temporary = tempdir()?;
    fs::create_dir_all(temporary.path().join("node_modules"))?;

    let output = cargo_bin_cmd!("devclean")
        .args(["scan", "--format", "json", "--redact-paths"])
        .arg(temporary.path())
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(output.status.success() && stdout.contains("<root:1>"));
    Ok(())
}

#[test]
fn watch_once_should_scan_without_cleaning() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    fs::write(modules.join("dependency.js"), "generated")?;

    cargo_bin_cmd!("devclean")
        .arg("watch")
        .args(["--once", "--no-notify", "--threshold", "1B"])
        .arg(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("watch scan: 1 candidates"))
        .stdout(predicate::str::contains("No cleanup").not());

    assert!(modules.is_dir());
    Ok(())
}

#[test]
fn clean_should_refuse_non_interactive_run_without_yes() -> Result<()> {
    let temporary = tempdir()?;
    fs::create_dir_all(temporary.path().join("node_modules"))?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg(temporary.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("without --yes"));
    Ok(())
}

#[test]
fn clean_should_remove_generated_directory_with_yes() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg(temporary.path())
        .assert()
        .success();

    assert!(!modules.exists());
    Ok(())
}

#[test]
fn clean_dry_run_should_print_plan_without_removing_candidate() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    fs::write(modules.join("dependency.js"), "generated")?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--dry-run")
        .arg(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dry run: planned 1 filesystem candidates",
        ))
        .stdout(predicate::str::contains("no changes made"));

    assert!(modules.is_dir());
    Ok(())
}

#[test]
fn clean_should_remove_only_exact_selected_path() -> Result<()> {
    let temporary = tempdir()?;
    let selected = temporary.path().join("selected/node_modules");
    let retained = temporary.path().join("retained/node_modules");
    fs::create_dir_all(&selected)?;
    fs::create_dir_all(&retained)?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg("--only-path")
        .arg(&selected)
        .arg(temporary.path())
        .assert()
        .success();

    assert!(!selected.exists());
    assert!(retained.exists());
    Ok(())
}

#[test]
fn clean_should_abort_when_exact_selected_path_is_stale() -> Result<()> {
    let temporary = tempdir()?;
    let retained = temporary.path().join("retained/node_modules");
    fs::create_dir_all(&retained)?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg("--only-path")
        .arg(temporary.path().join("missing/node_modules"))
        .arg(temporary.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no longer an eligible cleanup candidate",
        ));

    assert!(retained.exists());
    Ok(())
}

#[test]
fn scan_should_protect_git_tracked_candidate() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    fs::write(modules.join("vendored.js"), "tracked")?;
    Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(temporary.path())
        .status()?;
    Command::new("git")
        .args(["add", "node_modules/vendored.js"])
        .current_dir(temporary.path())
        .status()?;

    cargo_bin_cmd!("devclean")
        .args(["scan", "--format", "json"])
        .arg(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("protected Git-tracked candidate"));
    Ok(())
}

#[test]
fn scan_should_honor_config_exclude() -> Result<()> {
    let temporary = tempdir()?;
    fs::create_dir_all(temporary.path().join("vendor/node_modules"))?;
    let config = temporary.path().join("custom.toml");
    fs::write(&config, "[scan]\nexclude = [\"vendor/**\"]\n")?;

    cargo_bin_cmd!("devclean")
        .args(["scan", "--format", "json", "--config"])
        .arg(&config)
        .arg(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"candidates\": []"));
    Ok(())
}

#[test]
fn completions_should_generate_zsh_script() {
    cargo_bin_cmd!("devclean")
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef devclean"));
}

#[test]
fn init_should_create_strict_project_config_without_overwriting() -> Result<()> {
    let temporary = tempdir()?;
    let config = temporary.path().join(".devclean.toml");

    cargo_bin_cmd!("devclean")
        .arg("init")
        .arg("--output")
        .arg(&config)
        .assert()
        .success();
    cargo_bin_cmd!("devclean")
        .arg("init")
        .arg("--output")
        .arg(&config)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    assert!(fs::read_to_string(config)?.contains("[[rules]]"));
    Ok(())
}

#[test]
fn config_fetch_dry_run_should_not_clone_or_write() -> Result<()> {
    let temporary = tempdir()?;
    let output = temporary.path().join(".devclean.toml");

    cargo_bin_cmd!("devclean")
        .args([
            "config",
            "fetch",
            "https://example.invalid/team.git",
            "--dry-run",
        ])
        .arg("--output")
        .arg(&output)
        .assert()
        .success()
        .stdout(predicate::str::contains("would clone"));

    assert!(!output.exists());
    Ok(())
}

#[test]
fn stats_should_read_isolated_sqlite_history() -> Result<()> {
    let temporary = tempdir()?;
    let database = temporary.path().join("history.sqlite3");

    cargo_bin_cmd!("devclean")
        .args(["stats", "--format", "json", "--database"])
        .arg(&database)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"scan_count\": 0"));

    assert!(database.is_file());
    Ok(())
}

#[test]
fn schedule_install_should_require_authority_and_support_dry_run() {
    cargo_bin_cmd!("devclean")
        .args(["schedule", "install", "--every", "7d", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires --yes"));

    cargo_bin_cmd!("devclean")
        .args(["schedule", "install", "--every", "7d", "--yes", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--result-jsonl"));
}

#[test]
fn tui_snapshot_should_be_read_only_and_show_capacity_chart() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    fs::write(modules.join("dependency.js"), "generated")?;

    cargo_bin_cmd!("devclean")
        .args(["tui", "--snapshot"])
        .arg(temporary.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[ ]"))
        .stdout(predicate::str::contains("disk usage"));

    assert!(modules.is_dir());
    Ok(())
}

#[test]
fn manpage_should_generate_roff() {
    cargo_bin_cmd!("devclean")
        .arg("manpage")
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH devclean"));
}

#[test]
fn quarantine_should_list_and_restore_held_candidate() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    let registry = temporary.path().join("state/quarantine.json");

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .args(["--quarantine-for", "7d", "--quarantine-registry"])
        .arg(&registry)
        .arg(temporary.path())
        .assert()
        .success();

    let output = cargo_bin_cmd!("devclean")
        .args(["quarantine", "list", "--json", "--registry"])
        .arg(&registry)
        .output()?;
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let id = entries
        .first()
        .and_then(|entry| entry.get("id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("quarantine id missing"))?;

    cargo_bin_cmd!("devclean")
        .args(["quarantine", "restore", id, "--registry"])
        .arg(&registry)
        .assert()
        .success();

    assert!(modules.is_dir());
    Ok(())
}

#[test]
fn clean_undo_should_restore_selected_safety_hold() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    let registry = temporary.path().join("state/quarantine.json");

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .args(["--quarantine-for", "7d", "--quarantine-registry"])
        .arg(&registry)
        .arg(temporary.path())
        .assert()
        .success();

    let output = cargo_bin_cmd!("devclean")
        .args(["quarantine", "list", "--json", "--registry"])
        .arg(&registry)
        .output()?;
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let id = entries[0]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("quarantine id missing"))?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--undo")
        .arg(id)
        .arg("--quarantine-registry")
        .arg(&registry)
        .assert()
        .success()
        .stdout(predicate::str::contains("restored"));

    assert!(modules.is_dir());
    Ok(())
}

#[test]
fn quarantine_should_purge_one_selected_hold_immediately() -> Result<()> {
    let temporary = tempdir()?;
    let modules = temporary.path().join("node_modules");
    fs::create_dir_all(&modules)?;
    let registry = temporary.path().join("state/quarantine.json");

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .args(["--quarantine-for", "7d", "--quarantine-registry"])
        .arg(&registry)
        .arg(temporary.path())
        .assert()
        .success();

    let output = cargo_bin_cmd!("devclean")
        .args(["quarantine", "list", "--json", "--registry"])
        .arg(&registry)
        .output()?;
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let entry = entries
        .first()
        .ok_or_else(|| anyhow::anyhow!("quarantine entry missing"))?;
    let id = entry
        .get("id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("quarantine id missing"))?;
    let quarantine_path = entry
        .get("quarantine_path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("quarantine path missing"))?;

    cargo_bin_cmd!("devclean")
        .args(["quarantine", "purge", "--id", id, "--json", "--registry"])
        .arg(&registry)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"purged_bytes\""));

    assert!(!std::path::Path::new(quarantine_path).exists());
    assert!(!modules.exists());
    Ok(())
}

#[test]
fn clean_should_accept_only_scanner_recognized_approved_review_path() -> Result<()> {
    let temporary = tempdir()?;
    fs::write(
        temporary.path().join("Package.swift"),
        "// swift-tools-version: 6.0",
    )?;
    let build = temporary.path().join(".build");
    fs::create_dir_all(&build)?;
    fs::write(build.join("artifact"), "generated")?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg("--approve-review-path")
        .arg(&build)
        .arg("--only-path")
        .arg(&build)
        .arg(temporary.path())
        .assert()
        .success();

    assert!(!build.exists());
    Ok(())
}

#[test]
fn clean_should_require_cocoapods_lockfile_for_approved_pods() -> Result<()> {
    let temporary = tempdir()?;
    fs::write(temporary.path().join("Podfile"), "platform :ios")?;
    let pods = temporary.path().join("Pods");
    fs::create_dir_all(&pods)?;
    fs::write(pods.join("dependency.m"), "generated")?;

    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg("--approve-review-path")
        .arg(&pods)
        .arg("--only-path")
        .arg(&pods)
        .arg(temporary.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no longer an eligible cleanup candidate",
        ));
    assert!(pods.is_dir());

    fs::write(temporary.path().join("Podfile.lock"), "PODS:\n")?;
    cargo_bin_cmd!("devclean")
        .arg("clean")
        .arg("--yes")
        .arg("--approve-review-path")
        .arg(&pods)
        .arg("--only-path")
        .arg(&pods)
        .arg(temporary.path())
        .assert()
        .success();

    assert!(!pods.exists());
    Ok(())
}
