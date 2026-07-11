//! Safe discovery and cleanup of rebuildable development artifacts.

pub mod cleaner;
pub mod config;
pub mod docker;
pub mod model;
pub mod policy;
pub mod render;
pub mod scanner;

pub use cleaner::{CleanReport, clean};
pub use config::{Config, config_candidates, load_config, parse_age, parse_bytes};
pub use model::{Candidate, Category, OutputFormat, RenderOptions, ScanReport};
pub use render::{human_bytes, render, render_with_options};
pub use scanner::{ScanOptions, default_roots, scan};
