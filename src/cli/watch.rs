use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result};
use devclean::{ScanOptions, human_bytes, load_config, parse_age, parse_bytes, scan};
use notify::{RecursiveMode, Watcher};

use super::{WatchArgs, record_scan_history, scan_options, select_categories};

pub(super) fn run(arguments: &WatchArgs) -> Result<()> {
    let config = load_config(arguments.shared.config.as_deref())?;
    let categories = select_categories(&arguments.shared, true, false);
    let options = scan_options(&arguments.shared, &config, categories, false)?;
    let threshold = parse_bytes(
        arguments
            .threshold
            .as_deref()
            .unwrap_or(&config.watch.threshold),
    )?;
    let interval = parse_age(
        arguments
            .interval
            .as_deref()
            .unwrap_or(&config.watch.interval),
    )?;

    evaluate(&options, threshold, !arguments.no_notify)?;
    if arguments.once {
        return Ok(());
    }

    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(sender)?;
    for root in &options.roots {
        watcher.watch(root, RecursiveMode::Recursive)?;
    }
    println!(
        "watching {} roots; next scan occurs on a filesystem event, at most once per {}",
        options.roots.len(),
        humantime::format_duration(interval)
    );

    let mut last_scan: Option<Instant> = None;
    loop {
        receiver
            .recv()
            .context("filesystem watcher channel closed")?
            .context("filesystem watcher failed")?;
        if last_scan.is_some_and(|last| last.elapsed() < interval) {
            continue;
        }
        evaluate(&options, threshold, !arguments.no_notify)?;
        last_scan = Some(Instant::now());
    }
}

fn evaluate(options: &ScanOptions, threshold: u64, notify: bool) -> Result<()> {
    let report = scan(options)?;
    record_scan_history(&report);
    println!(
        "watch scan: {} candidates, {} reclaimable (threshold {})",
        report.candidates.len(),
        human_bytes(report.total_bytes),
        human_bytes(threshold)
    );
    if report.total_bytes >= threshold {
        println!("watch threshold reached; review with `devclean scan` before cleaning");
        if notify {
            send_desktop_notification(report.total_bytes);
        }
    }
    Ok(())
}

fn send_desktop_notification(bytes: u64) {
    let message = format!(
        "{} of rebuildable artifacts are ready for review. No cleanup was run.",
        human_bytes(bytes)
    );
    #[cfg(target_os = "macos")]
    let command = Command::new("osascript")
        .args([
            "-e",
            &format!("display notification \"{message}\" with title \"DevCleaner\""),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    #[cfg(target_os = "linux")]
    let command = Command::new("notify-send")
        .args(["DevCleaner", &message])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let _ = command;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let _ = message;
}
