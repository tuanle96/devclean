use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::{Args as ClapArgs, Subcommand};
use directories::BaseDirs;

const LABEL: &str = "dev.tuanle.devclean.schedule";

#[derive(Debug, ClapArgs)]
pub(super) struct Args {
    #[command(subcommand)]
    command: CommandArgs,
}

#[derive(Debug, Subcommand)]
enum CommandArgs {
    /// Install or replace the current user's recurring cleanup job.
    Install(InstallArgs),
    /// Show whether the recurring cleanup artifact exists.
    List,
    /// Disable and remove the recurring cleanup job.
    Remove,
}

#[derive(Debug, ClapArgs)]
struct InstallArgs {
    /// Cleanup cadence, for example 7d or 12h.
    #[arg(long)]
    every: String,
    /// Only clean artifacts older than this duration.
    #[arg(long, default_value = "30d")]
    older_than: String,
    /// Only clean artifacts at least this large.
    #[arg(long, default_value = "1GiB")]
    min_size: String,
    /// Include build and test outputs.
    #[arg(long)]
    all: bool,
    /// Explicitly authorize non-interactive cleanup in the installed job.
    #[arg(long)]
    yes: bool,
    /// Render the platform artifact without writing or registering it.
    #[arg(long)]
    dry_run: bool,
    /// Roots passed to each scheduled cleanup.
    #[arg(value_name = "ROOT")]
    roots: Vec<PathBuf>,
}

pub(super) fn run(arguments: &Args) -> Result<()> {
    match &arguments.command {
        CommandArgs::Install(arguments) => install(arguments),
        CommandArgs::List => list(),
        CommandArgs::Remove => remove(),
    }
}

fn install(arguments: &InstallArgs) -> Result<()> {
    if !arguments.yes {
        bail!("schedule install requires --yes to authorize future non-interactive cleanup");
    }
    let interval = devclean::parse_age(&arguments.every)?;
    let _ = devclean::parse_age(&arguments.older_than)?;
    let _ = devclean::parse_bytes(&arguments.min_size)?;
    if interval.as_secs() == 0 {
        bail!("--every must be greater than zero");
    }
    let executable = std::env::current_exe().context("failed to resolve devclean executable")?;
    let command = cleanup_arguments(arguments);

    install_platform(arguments, &executable, &command, interval.as_secs())
}

#[cfg(target_os = "macos")]
fn install_platform(
    arguments: &InstallArgs,
    executable: &Path,
    command: &[String],
    seconds: u64,
) -> Result<()> {
    install_launchd(arguments, executable, command, seconds)
}

#[cfg(target_os = "linux")]
fn install_platform(
    arguments: &InstallArgs,
    executable: &Path,
    command: &[String],
    _seconds: u64,
) -> Result<()> {
    install_systemd(arguments, executable, command)
}

#[cfg(target_os = "windows")]
fn install_platform(
    arguments: &InstallArgs,
    executable: &Path,
    command: &[String],
    seconds: u64,
) -> Result<()> {
    install_windows(arguments, executable, command, seconds)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn install_platform(
    _arguments: &InstallArgs,
    _executable: &Path,
    _command: &[String],
    _seconds: u64,
) -> Result<()> {
    bail!("scheduled cleanup is not supported on this platform")
}

fn cleanup_arguments(arguments: &InstallArgs) -> Vec<String> {
    let mut values = vec![
        "clean".to_owned(),
        "--yes".to_owned(),
        "--result-jsonl".to_owned(),
        "--older-than".to_owned(),
        arguments.older_than.clone(),
        "--min-size".to_owned(),
        arguments.min_size.clone(),
    ];
    if arguments.all {
        values.push("--all".to_owned());
    }
    values.extend(
        arguments
            .roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned()),
    );
    values
}

#[cfg(target_os = "macos")]
fn install_launchd(
    arguments: &InstallArgs,
    executable: &Path,
    command: &[String],
    interval: u64,
) -> Result<()> {
    let base = BaseDirs::new().context("home directory is unavailable")?;
    let path = base
        .home_dir()
        .join(format!("Library/LaunchAgents/{LABEL}.plist"));
    let log = base.home_dir().join("Library/Logs/devclean/schedule.jsonl");
    let mut program_arguments = format!("    <string>{}</string>\n", xml(executable));
    for argument in command {
        let _ = writeln!(program_arguments, "    <string>{}</string>", xml(argument));
    }
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n  <key>Label</key><string>{LABEL}</string>\n  <key>ProgramArguments</key><array>\n{program_arguments}  </array>\n  <key>StartInterval</key><integer>{interval}</integer>\n  <key>StandardOutPath</key><string>{}</string>\n  <key>StandardErrorPath</key><string>{}</string>\n</dict></plist>\n",
        xml(&log),
        xml(&log)
    );
    if arguments.dry_run {
        print!("{plist}");
        return Ok(());
    }
    fs::create_dir_all(path.parent().context("launch agent path has no parent")?)?;
    fs::create_dir_all(log.parent().context("schedule log path has no parent")?)?;
    fs::write(&path, plist)?;
    let domain = launchd_domain()?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&path)
        .stdin(Stdio::null())
        .status();
    run_status(
        Command::new("launchctl")
            .args(["bootstrap", &domain])
            .arg(&path),
    )?;
    println!("installed {}", path.display());
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_systemd(arguments: &InstallArgs, executable: &Path, command: &[String]) -> Result<()> {
    let base = BaseDirs::new().context("home directory is unavailable")?;
    let directory = base.config_dir().join("systemd/user");
    let service = directory.join("devclean-clean.service");
    let timer = directory.join("devclean-clean.timer");
    let execution = std::iter::once(executable.to_string_lossy().into_owned())
        .chain(command.iter().cloned())
        .map(|value| shell_quote(&value))
        .collect::<Vec<_>>()
        .join(" ");
    let service_text = format!(
        "[Unit]\nDescription=devclean scheduled cleanup\n[Service]\nType=oneshot\nExecStart={execution}\n"
    );
    let timer_text = format!(
        "[Unit]\nDescription=Run devclean every {}\n[Timer]\nOnBootSec=5m\nOnUnitActiveSec={}\nPersistent=true\n[Install]\nWantedBy=timers.target\n",
        arguments.every, arguments.every
    );
    if arguments.dry_run {
        print!("{service_text}\n{timer_text}");
        return Ok(());
    }
    fs::create_dir_all(&directory)?;
    fs::write(service, service_text)?;
    fs::write(timer, timer_text)?;
    run_status(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    run_status(Command::new("systemctl").args([
        "--user",
        "enable",
        "--now",
        "devclean-clean.timer",
    ]))?;
    println!("installed user systemd timer");
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_windows(
    arguments: &InstallArgs,
    executable: &Path,
    command: &[String],
    seconds: u64,
) -> Result<()> {
    let minutes = seconds.saturating_add(59) / 60;
    let task = std::iter::once(executable.to_string_lossy().into_owned())
        .chain(command.iter().cloned())
        .map(|value| format!("\"{}\"", value.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(" ");
    if arguments.dry_run {
        println!("schtasks /Create /TN {LABEL} /SC MINUTE /MO {minutes} /TR {task}");
        return Ok(());
    }
    run_status(Command::new("schtasks").args([
        "/Create",
        "/F",
        "/TN",
        LABEL,
        "/SC",
        "MINUTE",
        "/MO",
        &minutes.to_string(),
        "/TR",
        &task,
    ]))?;
    Ok(())
}

fn list() -> Result<()> {
    let path = schedule_path()?;
    println!(
        "{}\t{}",
        if path.exists() {
            "installed"
        } else {
            "not-installed"
        },
        path.display()
    );
    Ok(())
}

fn remove() -> Result<()> {
    let path = schedule_path()?;
    #[cfg(target_os = "macos")]
    if path.exists() {
        let domain = launchd_domain()?;
        let _ = Command::new("launchctl")
            .args(["bootout", &domain])
            .arg(&path)
            .status();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "devclean-clean.timer"])
            .status();
        let _ = fs::remove_file(path.with_file_name("devclean-clean.service"));
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("schtasks")
            .args(["/Delete", "/F", "/TN", LABEL])
            .status();
    }
    if path.exists() {
        fs::remove_file(&path)?;
    }
    println!("removed {}", path.display());
    Ok(())
}

fn schedule_path() -> Result<PathBuf> {
    let base = BaseDirs::new().context("home directory is unavailable")?;
    Ok(platform_schedule_path(&base))
}

#[cfg(target_os = "macos")]
fn platform_schedule_path(base: &BaseDirs) -> PathBuf {
    base.home_dir()
        .join(format!("Library/LaunchAgents/{LABEL}.plist"))
}

#[cfg(target_os = "linux")]
fn platform_schedule_path(base: &BaseDirs) -> PathBuf {
    base.config_dir().join("systemd/user/devclean-clean.timer")
}

#[cfg(target_os = "windows")]
fn platform_schedule_path(base: &BaseDirs) -> PathBuf {
    base.data_local_dir().join("devclean/schedule.task")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn platform_schedule_path(base: &BaseDirs) -> PathBuf {
    base.data_local_dir().join("devclean/schedule.unsupported")
}

fn run_status(command: &mut Command) -> Result<()> {
    let status = command.stdin(Stdio::null()).status()?;
    if !status.success() {
        bail!("scheduler command failed with {status}");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn launchd_domain() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .stdin(Stdio::null())
        .output()
        .context("failed to resolve current user id")?;
    if !output.status.success() {
        bail!("failed to resolve current user id");
    }
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

fn xml(value: impl AsRef<Path>) -> String {
    value
        .as_ref()
        .to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(target_os = "linux")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
