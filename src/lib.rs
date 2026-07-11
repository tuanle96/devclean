//! Safe discovery and cleanup of rebuildable development artifacts.

pub mod cleaner;
pub mod config;
pub mod docker;
pub mod model;
pub mod policy;
pub mod quarantine;
pub mod render;
pub mod scanner;

pub use cleaner::{CleanOptions, CleanReport, clean, clean_with_options};
pub use config::{Config, config_candidates, load_config, parse_age, parse_bytes};
pub use model::{
    Candidate, Category, Confidence, LearningObservation, OutputFormat, RenderOptions,
    ReviewCandidate, ScanReport,
};
pub use quarantine::{
    PurgeReport, QuarantineEntry, default_registry_path, hold, list as list_quarantine,
    purge_expired, restore as restore_quarantine,
};
pub use render::{human_bytes, render, render_with_options};
pub use scanner::{LearningMode, ScanOptions, default_roots, scan};
