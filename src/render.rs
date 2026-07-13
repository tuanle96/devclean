use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use minijinja::{Environment, context};
use serde::Serialize;
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
        OutputFormat::Html => render_html(&display_report),
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
    for observation in &report.learning_observations {
        writeln!(
            output,
            "{}",
            serde_json::to_string(
                &json!({"type": "learning_observation", "observation": observation})
            )?
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
            "learning_observation_count": report.learning_observations.len(),
            "observed_total_bytes": report.observed_total_bytes,
            "warnings": report.warnings,
        }))?
    )?;
    Ok(output)
}

#[derive(Serialize)]
struct HtmlCard {
    category: String,
    bytes: String,
}

#[derive(Serialize)]
struct HtmlRow {
    category: String,
    path: String,
    size: String,
    age: String,
    evidence: String,
}

fn render_html(report: &ScanReport) -> Result<String> {
    let cards = totals_by_category(report)
        .into_iter()
        .map(|(category, bytes)| HtmlCard {
            category: category.to_string(),
            bytes: human_bytes(bytes),
        })
        .collect::<Vec<_>>();
    let mut rows = report
        .candidates
        .iter()
        .map(|candidate| HtmlRow {
            category: candidate.category.to_string(),
            path: candidate.path.to_string_lossy().into_owned(),
            size: human_bytes(candidate.bytes),
            age: human_age(candidate.modified_at_unix),
            evidence: candidate.reason.clone(),
        })
        .collect::<Vec<_>>();
    rows.extend(report.review_candidates.iter().map(|candidate| HtmlRow {
        category: "review-only".to_owned(),
        path: candidate.path.to_string_lossy().into_owned(),
        size: human_bytes(candidate.bytes),
        age: human_age(candidate.modified_at_unix),
        evidence: candidate.reason.clone(),
    }));

    let mut environment = Environment::new();
    environment.add_template("report.html", include_str!("../templates/scan-report.html"))?;
    Ok(environment.get_template("report.html")?.render(context! {
        total_bytes => human_bytes(report.total_bytes),
        candidate_count => report.candidates.len(),
        review_total_bytes => human_bytes(report.review_total_bytes),
        review_candidate_count => report.review_candidates.len(),
        cards,
        rows,
        warnings => &report.warnings,
    })?)
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
                custom_rule: None,
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
    fn category_display_should_respect_column_width() {
        assert_eq!(format!("{:<23}", Category::NodeModules).len(), 23);
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
    fn render_jsonl_should_emit_learning_observations() -> Result<()> {
        use crate::model::LearningObservation;

        let mut input = report("/private/project/node_modules");
        input.learning_observations.push(LearningObservation {
            path: PathBuf::from("/private/project/target"),
            category: Some(Category::RustTarget),
            bytes: 4096,
            reason: "test".to_owned(),
            modified_at_unix: None,
            confidence: Confidence::Safe,
        });
        input.observed_total_bytes = 4096;

        let output = render(&input, OutputFormat::Jsonl)?;

        assert!(output.contains(r#""type":"learning_observation""#));
        assert!(output.contains(r#""learning_observation_count":1"#));
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
