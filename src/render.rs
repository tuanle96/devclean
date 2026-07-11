use std::fmt::Write as _;

use anyhow::Result;

use crate::model::{OutputFormat, ScanReport};
use crate::scanner::totals_by_category;

/// Renders a scan report in the requested format.
///
/// # Errors
///
/// Returns an error when JSON serialization fails.
pub fn render(report: &ScanReport, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Table => Ok(render_table(report)),
        OutputFormat::Json => Ok(serde_json::to_string_pretty(report)?),
        OutputFormat::Html => Ok(render_html(report)),
    }
}

/// Formats bytes using binary units.
#[must_use]
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut divisor = 1_u64;
    let mut unit = 0;
    while bytes / divisor >= 1024 && unit < UNITS.len() - 1 {
        divisor = divisor.saturating_mul(1024);
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        let tenths = (u128::from(bytes) * 10 + u128::from(divisor / 2)) / u128::from(divisor);
        format!("{}.{} {}", tenths / 10, tenths % 10, UNITS[unit])
    }
}

fn render_table(report: &ScanReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "{:<18} {:>12}  PATH", "CATEGORY", "SIZE");
    let _ = writeln!(output, "{}", "-".repeat(78));
    for candidate in &report.candidates {
        let _ = writeln!(
            output,
            "{:<18} {:>12}  {}",
            candidate.category,
            human_bytes(candidate.bytes),
            candidate.path.display()
        );
    }
    let _ = writeln!(output, "{}", "-".repeat(78));
    let _ = writeln!(
        output,
        "{} candidates, {} reclaimable (estimated)",
        report.candidates.len(),
        human_bytes(report.total_bytes)
    );
    for warning in &report.warnings {
        let _ = writeln!(output, "warning: {warning}");
    }
    output
}

fn render_html(report: &ScanReport) -> String {
    let totals = totals_by_category(report);
    let mut rows = String::new();
    for candidate in &report.candidates {
        let _ = writeln!(
            rows,
            "<tr><td><span class=\"tag\">{}</span></td><td class=\"size\">{}</td><td><code>{}</code></td><td>{}</td></tr>",
            candidate.category,
            human_bytes(candidate.bytes),
            escape_html(&candidate.path.display().to_string()),
            escape_html(&candidate.reason)
        );
    }
    let mut cards = String::new();
    let mut sorted_totals: Vec<_> = totals.into_iter().collect();
    sorted_totals.sort_by_key(|(category, _)| category.to_string());
    for (category, bytes) in sorted_totals {
        let _ = write!(
            cards,
            "<div class=\"card\"><span>{category}</span><strong>{}</strong></div>",
            human_bytes(bytes)
        );
    }
    let warnings = if report.warnings.is_empty() {
        "<p class=\"ok\">No traversal warnings.</p>".to_owned()
    } else {
        let mut items = String::new();
        for warning in &report.warnings {
            let _ = write!(items, "<li>{}</li>", escape_html(warning));
        }
        format!("<ul>{items}</ul>")
    };

    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>devclean scan report</title><style>
:root{{--bg:#08111f;--panel:#101c2d;--line:#23344d;--text:#e8eef7;--muted:#9eabc0;--accent:#65d6ad}}
*{{box-sizing:border-box}}body{{margin:0;background:linear-gradient(145deg,#07101d,#101a29);color:var(--text);font:15px/1.5 ui-sans-serif,system-ui;padding:40px}}
main{{max-width:1200px;margin:auto}}h1{{font-size:36px;margin:0 0 8px}}.lead{{color:var(--muted);margin:0 0 28px}}
.hero{{background:var(--panel);border:1px solid var(--line);border-radius:18px;padding:28px;margin-bottom:22px}}.hero strong{{color:var(--accent);font-size:38px;display:block}}
.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin:20px 0}}.card{{background:#0d1828;border:1px solid var(--line);padding:16px;border-radius:12px}}.card span{{color:var(--muted);display:block}}.card strong{{font-size:20px}}
.table-wrap{{overflow:auto;background:var(--panel);border:1px solid var(--line);border-radius:16px}}table{{width:100%;border-collapse:collapse}}th,td{{padding:13px 15px;text-align:left;border-bottom:1px solid var(--line)}}th{{color:var(--muted);font-size:12px;text-transform:uppercase}}.size{{white-space:nowrap;font-weight:700}}code{{color:#c5d8f2}}.tag{{color:var(--accent)}}.ok{{color:var(--accent)}}
</style></head><body><main><h1>devclean scan</h1><p class="lead">Read-only inventory of rebuildable development artifacts.</p>
<section class="hero"><span>Estimated reclaimable space</span><strong>{total}</strong><span>{count} candidates across {roots} roots</span><div class="cards">{cards}</div></section>
<div class="table-wrap"><table><thead><tr><th>Category</th><th>Size</th><th>Path</th><th>Evidence</th></tr></thead><tbody>{rows}</tbody></table></div>
<section><h2>Warnings</h2>{warnings}</section></main></body></html>"#,
        total = human_bytes(report.total_bytes),
        count = report.candidates.len(),
        roots = report.roots.len(),
    )
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
    use crate::model::ScanReport;

    #[test]
    fn human_bytes_should_use_binary_units() {
        assert_eq!(human_bytes(1_073_741_824), "1.0 GiB");
    }

    #[test]
    fn render_html_should_escape_candidate_paths() -> Result<()> {
        let report = ScanReport {
            roots: Vec::new(),
            candidates: vec![crate::Candidate {
                category: crate::Category::NodeModules,
                path: "a<b".into(),
                bytes: 1,
                reason: "test".to_owned(),
            }],
            warnings: Vec::new(),
            total_bytes: 1,
        };

        let html = render(&report, OutputFormat::Html)?;

        assert!(html.contains("a&lt;b"));
        Ok(())
    }
}
