//! Safe discovery and cleanup of rebuildable development artifacts.

pub mod cleaner;
pub mod docker;
pub mod model;
pub mod render;
pub mod scanner;

pub use cleaner::{CleanReport, clean};
pub use model::{Candidate, Category, OutputFormat, ScanReport};
pub use scanner::{ScanOptions, default_roots, scan};
