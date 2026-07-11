use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::json;

use crate::model::{OutputFormat, RenderOptions, ScanReport};
use crate::scanner::totals_by_category;

/// Renders a scan report without path redaction.
///
/// # Errors
///
/// Returns an error when JSON serialization fails.
pub fn render(report: &ScanReport, format: OutputFormat) -> Result<String> {
    render_with_options(report, format, RenderOptions::default())
}

/// Renders a scan report with presentation controls.
///
/// # Errors
///
/// Returns an error when JSON serialization fails.
pub fn render_with_options(
    report: &ScanReport,
    format: OutputFormat,
    options: RenderOptions,
) -> Result<String> {
    let display_report = if options.redact_paths {
        redact_report(report)
    } else {
        report.clone()
    };
    match format {
        OutputFormat::Table => Ok(render_table(&display_report)),
        OutputFormat::Json => Ok(serde_json::to_string_pretty(&display_report)?),
        OutputFormat::Jsonl => render_jsonl(&display_report),
        OutputFormat::Html => Ok(render_html(&display_report)),
    }
}

/// Formats bytes with binary units.
#[must_use]
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut unit = 0;
    let mut divisor = 1_u64;
    while bytes / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor = divisor.saturating_mul(1024);
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        let whole = bytes / divisor;
        let hundredths = bytes % divisor * 100 / divisor;
        format!("{whole}.{hundredths:02} {}", UNITS[unit])
    }
}

fn render_table(report: &ScanReport) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{:<23} {:>12} {:>10}  PATH",
        "CATEGORY", "SIZE", "AGE"
    );
    let _ = writeln!(output, "{}", "-".repeat(92));
    for candidate in &report.candidates {
        let _ = writeln!(
            output,
            "{:<23} {:>12} {:>10}  {}",
            candidate.category,
            human_bytes(candidate.bytes),
            human_age(candidate.modified_at_unix),
            candidate.path.display()
        );
    }
    for candidate in &report.review_candidates {
        let _ = writeln!(
            output,
            "{:<23} {:>12} {:>10}  {}",
            "review-only",
            human_bytes(candidate.bytes),
            human_age(candidate.modified_at_unix),
            candidate.path.display()
        );
    }
    let _ = writeln!(output, "{}", "-".repeat(92));
    let _ = writeln!(
        output,
        "{} candidates, {} reclaimable",
        report.candidates.len(),
        human_bytes(report.total_bytes)
    );
    if !report.review_candidates.is_empty() {
        let _ = writeln!(
            output,
            "{} review-only observations, {} watched",
            report.review_candidates.len(),
            human_bytes(report.review_total_bytes)
        );
    }
    for warning in &report.warnings {
        let _ = writeln!(output, "warning: {warning}");
    }
    output
}

fn render_jsonl(report: &ScanReport) -> Result<String> {
    let mut output = String::new();
    for candidate in &report.candidates {
        writeln!(
            output,
            "{}",
            serde_json::to_string(&json!({"type": "candidate", "candidate": candidate}))?
        )?;
    }
    for candidate in &report.review_candidates {
        writeln!(
            output,
            "{}",
            serde_json::to_string(&json!({"type": "review_candidate", "candidate": candidate}))?
        )?;
    }
    writeln!(
        output,
        "{}",
        serde_json::to_string(&json!({
            "type": "summary",
            "candidate_count": report.candidates.len(),
            "total_bytes": report.total_bytes,
            "review_candidate_count": report.review_candidates.len(),
            "review_total_bytes": report.review_total_bytes,
            "warnings": report.warnings,
        }))?
    )?;
    Ok(output)
}

fn render_html(report: &ScanReport) -> String {
    let totals = totals_by_category(report);
    let mut cards = String::new();
    for (category, bytes) in totals {
        let _ = write!(
            cards,
            "<article><span>{}</span><strong>{}</strong></article>",
            escape_html(&category.to_string()),
            human_bytes(bytes)
        );
    }

    let mut rows = String::new();
    for candidate in &report.candidates {
        let _ = write!(
            rows,
            "<tr><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&candidate.category.to_string()),
            escape_html(&candidate.path.to_string_lossy()),
            human_bytes(candidate.bytes),
            human_age(candidate.modified_at_unix),
            escape_html(&candidate.reason)
        );
    }
    for candidate in &report.review_candidates {
        let _ = write!(
            rows,
            "<tr><td>review-only</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&candidate.path.to_string_lossy()),
            human_bytes(candidate.bytes),
            human_age(candidate.modified_at_unix),
            escape_html(&candidate.reason)
        );
    }

    let mut warnings = String::new();
    for warning in &report.warnings {
        let _ = write!(warnings, "<li>{}</li>", escape_html(warning));
    }
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>devclean scan report</title><style>
  :root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui;background:#08111f;color:#ecf3ff}}body{{margin:0}}main{{max-width:1180px;margin:auto;padding:48px 24px}}h1{{font-size:42px;margin-bottom:8px}}.lead{{color:#aebdd2}}.summary{{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin:28px 0}}article{{background:#111d31;border:1px solid #263854;border-radius:14px;padding:16px}}article span{{display:block;color:#94a7c2;font-size:12px}}article strong{{font-size:22px}}.total{{color:#5fe0a5}}.review{{color:#ffcf72}}.table{{overflow:auto;border:1px solid #263854;border-radius:14px}}table{{width:100%;border-collapse:collapse;min-width:900px}}th,td{{padding:12px;text-align:left;border-bottom:1px solid #263854}}th{{background:#15233a;color:#cbd9ed}}td{{color:#b7c5da}}code{{color:#dbe9ff}}.warnings{{color:#ffcf72}}</style></head><body><main><h1>devclean scan</h1><p class="lead">Read-only inventory of rebuildable development artifacts.</p><p class="total"><strong>{}</strong> across {} safe candidates</p><p class="review"><strong>{}</strong> across {} review-only observations</p><section class="summary">{}</section><div class="table"><table><thead><tr><th>Category</th><th>Path</th><th>Size</th><th>Age</th><th>Evidence</th></tr></thead><tbody>{}</tbody></table></div><section class="warnings"><h2>Warnings</h2><ul>{}</ul></section></main></body></html>"#,
        human_bytes(report.total_bytes),
        report.candidates.len(),
        human_bytes(report.review_total_bytes),
        report.review_candidates.len(),
        cards,
        rows,
        warnings
    )
}

fn redact_report(report: &ScanReport) -> ScanReport {
    let mut redacted = report.clone();
    for candidate in &mut redacted.candidates {
        candidate.path = redact_path(&candidate.path, &report.roots);
    }
    for candidate in &mut redacted.review_candidates {
        candidate.path = redact_path(&candidate.path, &report.roots);
        candidate.project_root = candidate
            .project_root
            .as_deref()
            .map(|path| redact_path(path, &report.roots));
    }
    for observation in &mut redacted.learning_observations {
        observation.path = redact_path(&observation.path, &report.roots);
    }
    redacted.roots = report
        .roots
        .iter()
        .enumerate()
        .map(|(index, _)| PathBuf::from(format!("<root:{}>", index + 1)))
        .collect();
    redacted.warnings = report
        .warnings
        .iter()
        .map(|warning| redact_text(warning, &report.roots))
        .collect();
    redacted
}

fn redact_text(value: &str, roots: &[PathBuf]) -> String {
    let mut redacted = value.to_owned();
    for (index, root) in roots.iter().enumerate() {
        let root_text = root.to_string_lossy();
        redacted = redacted.replace(root_text.as_ref(), &format!("<root:{}>", index + 1));
    }
    if let Some(base) = directories::BaseDirs::new() {
        let home = base.home_dir().to_string_lossy();
        redacted = redacted.replace(home.as_ref(), "<home>");
    }
    redacted
}

fn redact_path(path: &Path, roots: &[PathBuf]) -> PathBuf {
    for (index, root) in roots.iter().enumerate() {
        if let Ok(relative) = path.strip_prefix(root) {
            return PathBuf::from(format!("<root:{}>", index + 1)).join(relative);
        }
    }
    if let Some(base) = directories::BaseDirs::new() {
        if let Ok(relative) = path.strip_prefix(base.home_dir()) {
            return PathBuf::from("<home>").join(relative);
        }
    }
    PathBuf::from("<external>").join(path.file_name().unwrap_or_default())
}

fn human_age(modified_at_unix: Option<u64>) -> String {
    let Some(modified) = modified_at_unix else {
        return "unknown".to_owned();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(modified, |duration| duration.as_secs());
    let seconds = now.saturating_sub(modified);
    if seconds >= 86_400 {
        format!("{}d", seconds / 86_400)
    } else if seconds >= 3_600 {
        format!("{}h", seconds / 3_600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Candidate, Category, Confidence, ReviewCandidate, ReviewRule, ScanReport};

    fn report(path: &str) -> ScanReport {
        ScanReport {
            roots: vec![PathBuf::from("/private/project")],
            candidates: vec![Candidate {
                category: Category::NodeModules,
                path: PathBuf::from(path),
                bytes: 1024,
                reason: "test".to_owned(),
                modified_at_unix: None,
                confidence: Confidence::Safe,
                approved_rule: None,
            }],
            review_candidates: Vec::new(),
            learning_observations: Vec::new(),
            warnings: Vec::new(),
            total_bytes: 1024,
            review_total_bytes: 0,
            observed_total_bytes: 0,
            protect_git_tracked: true,
        }
    }

    #[test]
    fn human_bytes_should_use_binary_units() {
        assert_eq!(human_bytes(1024), "1.00 KiB");
    }

    #[test]
    fn render_html_should_escape_candidate_paths() -> Result<()> {
        let output = render(&report("/tmp/<script>"), OutputFormat::Html)?;

        assert!(!output.contains("<script>"));
        Ok(())
    }

    #[test]
    fn render_should_redact_paths_in_json() -> Result<()> {
        let output = render_with_options(
            &report("/private/project/node_modules"),
            OutputFormat::Json,
            RenderOptions { redact_paths: true },
        )?;

        assert!(!output.contains("/private/project"));
        Ok(())
    }

    #[test]
    fn render_should_redact_review_project_root_in_json() -> Result<()> {
        let mut input = report("/private/project/node_modules");
        input.review_candidates.push(ReviewCandidate {
            path: PathBuf::from("/private/project/.build"),
            bytes: 2048,
            reason: "test".to_owned(),
            modified_at_unix: None,
            confidence: Confidence::Review,
            suggested_rule: Some(ReviewRule::SwiftPackageBuild),
            project_root: Some(PathBuf::from("/private/project")),
            approved: false,
        });

        let output = render_with_options(
            &input,
            OutputFormat::Json,
            RenderOptions { redact_paths: true },
        )?;

        assert!(!output.contains("/private/project"));
        Ok(())
    }

    #[test]
    fn render_should_redact_paths_inside_warnings() -> Result<()> {
        let mut input = report("/private/project/node_modules");
        input
            .warnings
            .push("protected /private/project/node_modules".to_owned());

        let output = render_with_options(
            &input,
            OutputFormat::Json,
            RenderOptions { redact_paths: true },
        )?;

        assert!(!output.contains("/private/project"));
        Ok(())
    }
}
