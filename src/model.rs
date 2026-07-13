use std::fmt;
use std::path::PathBuf;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// A class of rebuildable development data.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Cargo compilation outputs with Rust-specific markers.
    RustTarget,
    /// JavaScript dependency installations.
    NodeModules,
    /// Framework caches such as `.next` and `.svelte-kit`.
    FrameworkCache,
    /// Build directories underneath a recognized project manifest.
    BuildOutput,
    /// Test, mutation, type-checker, and lint caches.
    TestCache,
    /// Python bytecode, test-runner environments, and other reproducible interpreter caches.
    PythonCache,
    /// Project-local Python virtual environments with a direct dependency manifest.
    PythonEnvironment,
    /// Downloaded package-manager or tool caches.
    GlobalCache,
    /// Large runtimes or model caches that are expensive to restore.
    ExpensiveGlobalCache,
}

/// Confidence assigned to a filesystem observation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// A known rebuildable artifact with filesystem-verifiable evidence.
    #[default]
    Safe,
    /// A cache-like directory that must be reviewed before a cleanup rule is added.
    Review,
    /// A directory protected from cleanup because it may contain source or user data.
    Protected,
}

/// A narrowly-scoped cleanup rule that Learning Mode can propose for explicit approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewRule {
    /// A `.build` directory directly beside a Swift Package manifest.
    SwiftPackageBuild,
    /// A `DerivedData` directory directly beside an Xcode project or workspace.
    XcodeDerivedData,
    /// A `.gradle` directory directly beside a Gradle build or settings script.
    GradleBuild,
    /// A `Pods` directory directly beside both a `CocoaPods` `Podfile` and `Podfile.lock`.
    CocoaPods,
}

/// A declarative, config-defined cleanup rule with exact names and direct project markers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomRule {
    /// Stable rule name shown in reports.
    pub name: String,
    /// Existing cleanup category used for filtering and presentation.
    pub category: Category,
    /// Exact directory names this rule may classify.
    pub directory_names: Vec<String>,
    /// Files that must exist directly beside the candidate.
    pub required_markers: Vec<String>,
    /// Human-readable evidence shown in cleanup plans.
    pub reason: String,
}

impl ReviewRule {
    /// Returns every rule Learning Mode can suggest, in suggestion order.
    #[must_use]
    pub fn all() -> [Self; 4] {
        [
            Self::SwiftPackageBuild,
            Self::XcodeDerivedData,
            Self::GradleBuild,
            Self::CocoaPods,
        ]
    }
}

impl Category {
    /// Returns all categories discovered by a comprehensive scan.
    #[must_use]
    pub fn all() -> [Self; 9] {
        [
            Self::RustTarget,
            Self::NodeModules,
            Self::FrameworkCache,
            Self::BuildOutput,
            Self::TestCache,
            Self::PythonCache,
            Self::PythonEnvironment,
            Self::GlobalCache,
            Self::ExpensiveGlobalCache,
        ]
    }

    /// Returns conservative categories used by `clean` unless overridden.
    #[must_use]
    pub fn safe_defaults() -> [Self; 5] {
        [
            Self::RustTarget,
            Self::NodeModules,
            Self::FrameworkCache,
            Self::PythonCache,
            Self::PythonEnvironment,
        ]
    }
}

impl fmt::Display for Category {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::RustTarget => "rust-target",
            Self::NodeModules => "node-modules",
            Self::FrameworkCache => "framework-cache",
            Self::BuildOutput => "build-output",
            Self::TestCache => "test-cache",
            Self::PythonCache => "python-cache",
            Self::PythonEnvironment => "python-environment",
            Self::GlobalCache => "global-cache",
            Self::ExpensiveGlobalCache => "expensive-global-cache",
        };
        formatter.pad(value)
    }
}

/// A directory that can be rebuilt or downloaded again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Artifact category.
    pub category: Category,
    /// Absolute or user-supplied path to the artifact.
    pub path: PathBuf,
    /// Estimated allocated bytes on disk.
    pub bytes: u64,
    /// Evidence used to classify the directory.
    pub reason: String,
    /// Latest observed modification time as seconds since the Unix epoch.
    pub modified_at_unix: Option<u64>,
    /// Safety confidence assigned by the scanner.
    #[serde(default)]
    pub confidence: Confidence,
    /// Explicit learned rule that authorized this candidate, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_rule: Option<ReviewRule>,
    /// Declarative rule that classified this candidate, if configured by the user or team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_rule: Option<CustomRule>,
}

/// A large cache-like directory observed by Learning Mode but never selected for cleanup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCandidate {
    /// Absolute path retained only in the local report.
    pub path: PathBuf,
    /// Estimated allocated bytes on disk.
    pub bytes: u64,
    /// Evidence that made the directory interesting to Learning Mode.
    pub reason: String,
    /// Latest observed modification time as seconds since the Unix epoch.
    pub modified_at_unix: Option<u64>,
    /// Review candidates are never promoted to safe without an explicit product rule.
    pub confidence: Confidence,
    /// Product rule that can safely constrain an approval for this path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_rule: Option<ReviewRule>,
    /// Project root to which the suggested rule would be scoped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<PathBuf>,
    /// Whether the user has approved the suggested rule for this exact path.
    #[serde(default)]
    pub approved: bool,
}

/// One local-only Learning Mode measurement independent of cleanup eligibility filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningObservation {
    /// Observed artifact path. Remote telemetry must never include this value.
    pub path: PathBuf,
    /// Known category when the scanner can classify the artifact safely.
    pub category: Option<Category>,
    /// Estimated allocated bytes on disk.
    pub bytes: u64,
    /// Filesystem evidence behind the observation.
    pub reason: String,
    /// Latest observed modification time as seconds since the Unix epoch.
    pub modified_at_unix: Option<u64>,
    /// Safety confidence independent of cleanup age and size filters.
    pub confidence: Confidence,
}

/// Result of scanning one or more roots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    /// Roots traversed by the scanner.
    pub roots: Vec<PathBuf>,
    /// Rebuildable directories found.
    pub candidates: Vec<Candidate>,
    /// Cache-like directories that require review and cannot be cleaned yet.
    #[serde(default)]
    pub review_candidates: Vec<ReviewCandidate>,
    /// Local measurements used for growth history, including active artifacts filtered from cleanup.
    #[serde(default)]
    pub learning_observations: Vec<LearningObservation>,
    /// Non-fatal traversal or metadata errors.
    pub warnings: Vec<String>,
    /// Total estimated allocated bytes for all candidates.
    pub total_bytes: u64,
    /// Total allocated bytes represented by review-only observations.
    #[serde(default)]
    pub review_total_bytes: u64,
    /// Total allocated bytes represented by Learning Mode measurements.
    #[serde(default)]
    pub observed_total_bytes: u64,
    /// Whether cleanup must repeat the Git tracked-file guard.
    #[serde(default = "default_true")]
    pub protect_git_tracked: bool,
}

const fn default_true() -> bool {
    true
}

/// Human- or machine-readable report format.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Compact terminal table.
    Table,
    /// Structured JSON.
    Json,
    /// One JSON object per line for streaming automation.
    Jsonl,
    /// Standalone HTML document.
    Html,
}

/// Controls presentation without changing the underlying scan result.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    /// Replace absolute paths with stable root-relative placeholders.
    pub redact_paths: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_rule_wire_names_should_match_the_macos_app_contract() {
        let encoded: Vec<String> = ReviewRule::all()
            .iter()
            .map(|rule| serde_json::to_string(rule).expect("rule serializes"))
            .collect();

        assert_eq!(
            encoded,
            [
                r#""swift-package-build""#,
                r#""xcode-derived-data""#,
                r#""gradle-build""#,
                r#""cocoa-pods""#,
            ]
        );
    }
}
