//! Persistent safety holds for artifacts that should remain restorable before deletion.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use fs4::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::Category;

const STORE_VERSION: u32 = 1;
const QUARANTINE_PREFIX: &str = ".devclean-quarantine-";

/// One restorable artifact held beside its original location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    /// Stable identifier used by list, restore, and purge commands.
    pub id: String,
    /// Original artifact location.
    pub original_path: PathBuf,
    /// Hidden adjacent location used during the safety hold.
    pub quarantine_path: PathBuf,
    /// Scanner category validated immediately before the move.
    pub category: Category,
    /// Scan-time allocated bytes retained by the hold.
    pub bytes: u64,
    /// Creation time as seconds since the Unix epoch.
    pub created_at_unix: u64,
    /// Time after which automatic purge is allowed.
    pub expires_at_unix: u64,
}

/// Outcome of purging expired or explicitly selected safety holds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PurgeReport {
    /// Holds deleted successfully.
    pub purged: Vec<QuarantineEntry>,
    /// Hold-specific failures. Other valid holds are still processed.
    pub failures: Vec<String>,
    /// Allocated bytes represented by successful purges.
    pub purged_bytes: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct QuarantineStore {
    version: u32,
    entries: Vec<QuarantineEntry>,
}

/// Returns the platform-local registry used to track safety holds.
///
/// # Errors
///
/// Returns an error when the operating system data directory is unavailable.
pub fn default_registry_path() -> Result<PathBuf> {
    let base = BaseDirs::new().context("platform data directory is unavailable")?;
    Ok(base.data_local_dir().join("devclean/quarantine.json"))
}

/// Moves a validated artifact into a hidden adjacent safety hold and records it.
///
/// The move does not reclaim disk space until the hold is purged.
///
/// # Errors
///
/// Returns an error if the path is not a real directory, cannot be moved atomically, or the
/// registry cannot be updated. A failed registry update attempts to restore the original path.
pub fn hold(
    path: &Path,
    category: Category,
    bytes: u64,
    retention: Duration,
    registry_path: Option<&Path>,
) -> Result<QuarantineEntry> {
    if retention.is_zero() {
        bail!("quarantine retention must be greater than zero");
    }
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect candidate {}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("candidate is not a real directory");
    }
    let parent = path.parent().context("candidate has no parent directory")?;
    let now = unix_time(SystemTime::now());
    let id = next_id();
    let quarantine_path = parent.join(format!("{QUARANTINE_PREFIX}{id}"));
    if quarantine_path.exists() {
        bail!("unique quarantine path unexpectedly exists");
    }

    fs::rename(path, &quarantine_path).with_context(|| {
        format!(
            "failed to move {} into an adjacent safety hold",
            path.display()
        )
    })?;
    let entry = QuarantineEntry {
        id,
        original_path: path.to_path_buf(),
        quarantine_path: quarantine_path.clone(),
        category,
        bytes,
        created_at_unix: now,
        expires_at_unix: now.saturating_add(retention.as_secs()),
    };

    let registry =
        registry_path.map_or_else(default_registry_path, |value| Ok(value.to_path_buf()));
    if let Err(error) = registry.and_then(|registry| {
        update_store(&registry, |store| {
            store.entries.push(entry.clone());
            Ok(())
        })
    }) {
        let restored = fs::rename(&quarantine_path, path).is_ok();
        bail!("failed to record safety hold: {error:#}; restored original path: {restored}");
    }
    Ok(entry)
}

/// Lists all recorded safety holds after pruning registry entries whose paths disappeared.
///
/// # Errors
///
/// Returns an error when the registry is unreadable or invalid.
pub fn list(registry_path: Option<&Path>) -> Result<Vec<QuarantineEntry>> {
    let registry =
        registry_path.map_or_else(default_registry_path, |value| Ok(value.to_path_buf()))?;
    let mut entries = Vec::new();
    update_store(&registry, |store| {
        store.entries.retain(|entry| entry.quarantine_path.exists());
        entries.clone_from(&store.entries);
        Ok(())
    })?;
    entries.sort_by_key(|entry| entry.expires_at_unix);
    Ok(entries)
}

/// Restores one safety hold to its original location.
///
/// # Errors
///
/// Returns an error for an unknown identifier, an unsafe registry entry, an occupied original
/// path, or a failed filesystem move.
pub fn restore(id: &str, registry_path: Option<&Path>) -> Result<QuarantineEntry> {
    let registry =
        registry_path.map_or_else(default_registry_path, |value| Ok(value.to_path_buf()))?;
    let mut restored = None;
    update_store(&registry, |store| {
        let index = store
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .with_context(|| format!("unknown quarantine id `{id}`"))?;
        let entry = store.entries[index].clone();
        validate_entry(&entry)?;
        if entry.original_path.exists() {
            bail!(
                "cannot restore because original path exists: {}",
                entry.original_path.display()
            );
        }
        fs::rename(&entry.quarantine_path, &entry.original_path)
            .with_context(|| format!("failed to restore {}", entry.original_path.display()))?;
        store.entries.remove(index);
        restored = Some(entry);
        Ok(())
    })?;
    restored.context("quarantine restore completed without an entry")
}

/// Purges holds whose expiration time is at or before `now_unix`.
///
/// # Errors
///
/// Returns an error when the registry cannot be loaded or saved. Individual deletion failures
/// are returned in [`PurgeReport::failures`].
pub fn purge_expired(now_unix: u64, registry_path: Option<&Path>) -> Result<PurgeReport> {
    let registry =
        registry_path.map_or_else(default_registry_path, |value| Ok(value.to_path_buf()))?;
    let mut report = PurgeReport::default();
    update_store(&registry, |store| {
        let mut retained = Vec::new();
        for entry in store.entries.drain(..) {
            if entry.expires_at_unix > now_unix {
                retained.push(entry);
                continue;
            }
            if !entry.quarantine_path.exists() {
                continue;
            }
            if let Err(error) = validate_entry(&entry)
                .and_then(|()| fs::remove_dir_all(&entry.quarantine_path).map_err(Into::into))
            {
                report
                    .failures
                    .push(format!("{}: {error:#}", entry.quarantine_path.display()));
                retained.push(entry);
                continue;
            }
            report.purged_bytes = report.purged_bytes.saturating_add(entry.bytes);
            report.purged.push(entry);
        }
        store.entries = retained;
        Ok(())
    })?;
    Ok(report)
}

/// Permanently deletes one explicitly selected safety hold before or after expiry.
///
/// # Errors
///
/// Returns an error when the identifier is unknown or the registry cannot be loaded or saved.
/// A validation or filesystem deletion failure is returned in [`PurgeReport::failures`].
pub fn purge_selected(id: &str, registry_path: Option<&Path>) -> Result<PurgeReport> {
    let registry =
        registry_path.map_or_else(default_registry_path, |value| Ok(value.to_path_buf()))?;
    let mut report = PurgeReport::default();
    update_store(&registry, |store| {
        let index = store
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .with_context(|| format!("unknown quarantine id `{id}`"))?;
        let entry = store.entries[index].clone();
        if !entry.quarantine_path.exists() {
            store.entries.remove(index);
            return Ok(());
        }
        if let Err(error) = validate_entry(&entry)
            .and_then(|()| fs::remove_dir_all(&entry.quarantine_path).map_err(Into::into))
        {
            report
                .failures
                .push(format!("{}: {error:#}", entry.quarantine_path.display()));
            return Ok(());
        }
        store.entries.remove(index);
        report.purged_bytes = entry.bytes;
        report.purged.push(entry);
        Ok(())
    })?;
    Ok(report)
}

fn validate_entry(entry: &QuarantineEntry) -> Result<()> {
    let expected_parent = entry
        .original_path
        .parent()
        .context("recorded original path has no parent")?;
    let expected_name = format!("{QUARANTINE_PREFIX}{}", entry.id);
    if entry.quarantine_path.parent() != Some(expected_parent)
        || entry.quarantine_path.file_name() != Some(OsStr::new(&expected_name))
    {
        bail!("registry entry does not point to an adjacent devclean quarantine");
    }
    let metadata = fs::symlink_metadata(&entry.quarantine_path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("quarantine path is not a real directory");
    }
    Ok(())
}

fn update_store(
    registry_path: &Path,
    operation: impl FnOnce(&mut QuarantineStore) -> Result<()>,
) -> Result<()> {
    let parent = registry_path
        .parent()
        .context("quarantine registry has no parent directory")?;
    fs::create_dir_all(parent)?;
    let lock_path = registry_path.with_extension("lock");
    let lock = open_private(&lock_path)?;
    FileExt::lock(&lock)?;

    let mut store = if registry_path.is_file() {
        let content = fs::read(registry_path)?;
        serde_json::from_slice(&content).context("invalid quarantine registry")?
    } else {
        QuarantineStore {
            version: STORE_VERSION,
            entries: Vec::new(),
        }
    };
    if store.version != STORE_VERSION {
        bail!("unsupported quarantine registry version {}", store.version);
    }
    operation(&mut store)?;
    save_store(registry_path, &store)?;
    FileExt::unlock(&lock)?;
    Ok(())
}

fn save_store(path: &Path, store: &QuarantineStore) -> Result<()> {
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .with_context(|| format!("failed to open {}", temporary.display()))?;
    serde_json::to_writer_pretty(&mut file, store)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

fn open_private(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))
}

fn next_id() -> String {
    Uuid::new_v4().to_string()
}

fn unix_time(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn hold_and_restore_should_round_trip_directory() -> Result<()> {
        let temporary = tempdir()?;
        let original = temporary.path().join("node_modules");
        fs::create_dir_all(&original)?;
        let registry = temporary.path().join("state/quarantine.json");

        let entry = hold(
            &original,
            Category::NodeModules,
            42,
            Duration::from_secs(60),
            Some(&registry),
        )?;
        let restored = restore(&entry.id, Some(&registry))?;

        assert_eq!(restored.original_path, original);
        assert!(original.is_dir());
        Ok(())
    }

    #[test]
    fn purge_should_delete_expired_hold() -> Result<()> {
        let temporary = tempdir()?;
        let original = temporary.path().join("target");
        fs::create_dir_all(&original)?;
        let registry = temporary.path().join("state/quarantine.json");
        let entry = hold(
            &original,
            Category::RustTarget,
            99,
            Duration::from_secs(1),
            Some(&registry),
        )?;

        let report = purge_expired(u64::MAX, Some(&registry))?;

        assert_eq!(report.purged_bytes, 99);
        assert!(!entry.quarantine_path.exists());
        Ok(())
    }

    #[test]
    fn purge_selected_should_delete_only_requested_hold() -> Result<()> {
        let temporary = tempdir()?;
        let first = temporary.path().join("first/target");
        let second = temporary.path().join("second/target");
        fs::create_dir_all(&first)?;
        fs::create_dir_all(&second)?;
        let registry = temporary.path().join("state/quarantine.json");
        let first_entry = hold(
            &first,
            Category::RustTarget,
            40,
            Duration::from_secs(60),
            Some(&registry),
        )?;
        let second_entry = hold(
            &second,
            Category::RustTarget,
            60,
            Duration::from_secs(60),
            Some(&registry),
        )?;

        let report = purge_selected(&first_entry.id, Some(&registry))?;

        assert_eq!(report.purged_bytes, 40);
        assert!(!first_entry.quarantine_path.exists());
        assert!(second_entry.quarantine_path.exists());
        assert_eq!(list(Some(&registry))?.len(), 1);
        Ok(())
    }

    #[test]
    fn purge_selected_should_refuse_unknown_id_without_deleting_holds() -> Result<()> {
        let temporary = tempdir()?;
        let original = temporary.path().join("target");
        fs::create_dir_all(&original)?;
        let registry = temporary.path().join("state/quarantine.json");
        let entry = hold(
            &original,
            Category::RustTarget,
            99,
            Duration::from_secs(60),
            Some(&registry),
        )?;

        let result = purge_selected("missing", Some(&registry));

        assert!(result.is_err());
        assert!(entry.quarantine_path.exists());
        assert_eq!(list(Some(&registry))?.len(), 1);
        Ok(())
    }

    #[test]
    fn restore_should_refuse_occupied_original_path() -> Result<()> {
        let temporary = tempdir()?;
        let original = temporary.path().join("node_modules");
        fs::create_dir_all(&original)?;
        let registry = temporary.path().join("state/quarantine.json");
        let entry = hold(
            &original,
            Category::NodeModules,
            42,
            Duration::from_secs(60),
            Some(&registry),
        )?;
        fs::create_dir_all(&original)?;

        let result = restore(&entry.id, Some(&registry));

        assert!(result.is_err());
        Ok(())
    }
}
