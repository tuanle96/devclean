//! Safe discovery and cleanup of rebuildable development artifacts.

pub mod analysis;
pub mod cleaner;
pub mod config;
pub mod docker;
pub mod history;
pub mod model;
pub mod policy;
pub mod quarantine;
pub mod render;
pub mod scanner;
pub mod workspace;

pub use analysis::{AnalysisInsight, AnalysisReport, InsightKind, InsightSeverity, analyze};
pub use cleaner::{CleanOptions, CleanReport, clean, clean_with_options};
pub use config::{Config, config_candidates, load_config, parse_age, parse_bytes};
pub use model::{
    Candidate, Category, Confidence, CustomRule, LearningObservation, OutputFormat, RenderOptions,
    ReviewCandidate, ReviewRule, ScanReport,
};
pub use quarantine::{
    PurgeReport, QuarantineEntry, default_registry_path, hold, list as list_quarantine,
    purge_expired, purge_selected, restore as restore_quarantine,
};
pub use render::{human_bytes, render, render_with_options};
pub use scanner::{LearningMode, ScanOptions, default_roots, scan};
pub use workspace::{WorkspaceKind, WorkspaceSummary};
