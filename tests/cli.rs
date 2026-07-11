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
fn manpage_should_generate_roff() {
    cargo_bin_cmd!("devclean")
        .arg("manpage")
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH devclean"));
}
