//! Local `SQLite` history for scan trends and cleanup outcomes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};
use directories::BaseDirs;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::cleaner::CleanReport;
use crate::model::{Category, ScanReport};
use crate::scanner::totals_by_category;

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct HistorySummary {
    pub days: u64,
    pub scan_count: u64,
    pub cleanup_count: u64,
    pub latest_reclaimable_bytes: u64,
    pub reclaimable_change_bytes: i64,
    pub removed_bytes: u64,
    pub quarantined_bytes: u64,
    pub failures: u64,
    pub category_change_bytes: BTreeMap<Category, i64>,
    /// Number of consecutive scan snapshots in which each category grew.
    pub category_growth_events: BTreeMap<Category, u64>,
}

/// Returns the platform-local history database path.
///
/// # Errors
///
/// Returns an error when the platform data directory is unavailable.
pub fn default_database_path() -> Result<PathBuf> {
    let base = BaseDirs::new().context("platform data directory is unavailable")?;
    Ok(base.data_local_dir().join("devclean/history.sqlite3"))
}

/// Records a privacy-local aggregate scan event.
///
/// # Errors
///
/// Returns an error when the database cannot be opened, migrated, or written.
pub fn record_scan(report: &ScanReport, path: Option<&Path>) -> Result<()> {
    let connection = open(path)?;
    let categories = serde_json::to_string(&totals_by_category(report))?;
    connection.execute(
        "INSERT INTO scan_events(at_unix,total_bytes,candidate_count,categories_json) VALUES (?1,?2,?3,?4)",
        params![to_i64(now_unix()), to_i64(report.total_bytes), to_i64(report.candidates.len() as u64), categories],
    )?;
    Ok(())
}

/// Records one cleanup outcome without storing candidate paths.
///
/// # Errors
///
/// Returns an error when the database cannot be opened, migrated, or written.
pub fn record_cleanup(report: &CleanReport, path: Option<&Path>) -> Result<()> {
    let connection = open(path)?;
    connection.execute(
        "INSERT INTO cleanup_events(at_unix,removed_bytes,quarantined_bytes,removed_count,held_count,failures) VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            to_i64(now_unix()),
            to_i64(report.removed_bytes),
            to_i64(report.quarantined_bytes),
            to_i64(report.removed.len() as u64),
            to_i64(report.quarantined.len() as u64),
            to_i64(report.failures.len() as u64),
        ],
    )?;
    Ok(())
}

/// Summarizes scan growth and cleanup outcomes over a rolling window.
///
/// # Errors
///
/// Returns an error when the database cannot be opened or queried.
pub fn summarize(days: u64, path: Option<&Path>) -> Result<HistorySummary> {
    let connection = open(path)?;
    let since = now_unix().saturating_sub(days.saturating_mul(86_400));
    let scan_count = query_u64(
        &connection,
        "SELECT COUNT(*) FROM scan_events WHERE at_unix >= ?1",
        since,
    )?;
    let cleanup_count = query_u64(
        &connection,
        "SELECT COUNT(*) FROM cleanup_events WHERE at_unix >= ?1",
        since,
    )?;
    let cleanup_totals = connection.query_row(
        "SELECT COALESCE(SUM(removed_bytes),0),COALESCE(SUM(quarantined_bytes),0),COALESCE(SUM(failures),0) FROM cleanup_events WHERE at_unix >= ?1",
        [to_i64(since)],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
    )?;
    let first = scan_snapshot(&connection, since, SortOrder::Ascending)?;
    let latest = scan_snapshot(&connection, since, SortOrder::Descending)?;
    let latest_bytes = latest.as_ref().map_or(0, |snapshot| snapshot.0);
    let first_bytes = first.as_ref().map_or(latest_bytes, |snapshot| snapshot.0);
    let category_change_bytes = category_delta(
        first.as_ref().map(|snapshot| &snapshot.1),
        latest.as_ref().map(|snapshot| &snapshot.1),
    );

    Ok(HistorySummary {
        days,
        scan_count,
        cleanup_count,
        latest_reclaimable_bytes: latest_bytes,
        reclaimable_change_bytes: signed_delta(latest_bytes, first_bytes),
        removed_bytes: from_i64(cleanup_totals.0),
        quarantined_bytes: from_i64(cleanup_totals.1),
        failures: from_i64(cleanup_totals.2),
        category_change_bytes,
        category_growth_events: category_growth_events(&connection, since)?,
    })
}

fn open(path: Option<&Path>) -> Result<Connection> {
    let path = path.map_or_else(default_database_path, |value| Ok(value.to_path_buf()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut connection = Connection::open(&path)
        .with_context(|| format!("failed to open history database {}", path.display()))?;
    connection.execute_batch("PRAGMA journal_mode=WAL;")?;
    migrate(&mut connection)?;
    Ok(connection)
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let version = schema_version(connection)?;
    ensure!(
        version <= CURRENT_SCHEMA_VERSION,
        "history database schema version {version} is newer than supported version {CURRENT_SCHEMA_VERSION}"
    );

    if version == 0 {
        migrate_v0_to_v1(connection)?;
    }
    Ok(())
}

fn migrate_v0_to_v1(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS scan_events(id INTEGER PRIMARY KEY,at_unix INTEGER NOT NULL,total_bytes INTEGER NOT NULL,candidate_count INTEGER NOT NULL,categories_json TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS scan_events_at ON scan_events(at_unix);
         CREATE TABLE IF NOT EXISTS cleanup_events(id INTEGER PRIMARY KEY,at_unix INTEGER NOT NULL,removed_bytes INTEGER NOT NULL,quarantined_bytes INTEGER NOT NULL,removed_count INTEGER NOT NULL,held_count INTEGER NOT NULL,failures INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS cleanup_events_at ON cleanup_events(at_unix);
         PRAGMA user_version=1;",
    )?;
    transaction.commit()?;
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<u32> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

fn scan_snapshot(
    connection: &Connection,
    since: u64,
    order: SortOrder,
) -> Result<Option<(u64, BTreeMap<Category, u64>)>> {
    let order = order.sql();
    let sql = format!(
        "SELECT total_bytes,categories_json FROM scan_events WHERE at_unix >= ?1 ORDER BY at_unix {order},id {order} LIMIT 1"
    );
    let raw = connection
        .query_row(&sql, [to_i64(since)], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .optional()?;
    raw.map(|(bytes, categories)| {
        Ok((
            from_i64(bytes),
            serde_json::from_str::<BTreeMap<Category, u64>>(&categories)?,
        ))
    })
    .transpose()
}

#[derive(Debug, Clone, Copy)]
enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    const fn sql(self) -> &'static str {
        match self {
            Self::Ascending => "ASC",
            Self::Descending => "DESC",
        }
    }
}

fn category_growth_events(connection: &Connection, since: u64) -> Result<BTreeMap<Category, u64>> {
    let mut statement = connection.prepare(
        "SELECT categories_json FROM scan_events WHERE at_unix >= ?1 ORDER BY at_unix ASC,id ASC",
    )?;
    let snapshots = statement.query_map([to_i64(since)], |row| row.get::<_, String>(0))?;
    let mut previous: Option<BTreeMap<Category, u64>> = None;
    let mut growth = BTreeMap::<Category, u64>::new();
    for snapshot in snapshots {
        let current = serde_json::from_str::<BTreeMap<Category, u64>>(&snapshot?)?;
        if let Some(previous) = &previous {
            for category in Category::all() {
                let old = previous.get(&category).copied().unwrap_or(0);
                let new = current.get(&category).copied().unwrap_or(0);
                if new > old {
                    let count = growth.entry(category).or_default();
                    *count = count.saturating_add(1);
                }
            }
        }
        previous = Some(current);
    }
    Ok(growth)
}

fn category_delta(
    first: Option<&BTreeMap<Category, u64>>,
    latest: Option<&BTreeMap<Category, u64>>,
) -> BTreeMap<Category, i64> {
    Category::all()
        .into_iter()
        .filter_map(|category| {
            let old = first
                .and_then(|values| values.get(&category))
                .copied()
                .unwrap_or(0);
            let new = latest
                .and_then(|values| values.get(&category))
                .copied()
                .unwrap_or(0);
            let delta = signed_delta(new, old);
            (delta != 0).then_some((category, delta))
        })
        .collect()
}

fn query_u64(connection: &Connection, sql: &str, since: u64) -> Result<u64> {
    Ok(from_i64(connection.query_row(
        sql,
        [to_i64(since)],
        |row| row.get(0),
    )?))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn signed_delta(new: u64, old: u64) -> i64 {
    i64::try_from(
        i128::from(new)
            .saturating_sub(i128::from(old))
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)),
    )
    .expect("clamped delta always fits in i64")
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn from_i64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::model::{Candidate, Confidence};

    #[test]
    fn history_should_record_aggregate_scan_without_paths() -> Result<()> {
        let temporary = tempdir()?;
        let database = temporary.path().join("history.sqlite3");
        let report = ScanReport {
            roots: vec![temporary.path().to_path_buf()],
            candidates: vec![Candidate {
                category: Category::NodeModules,
                path: temporary.path().join("private/node_modules"),
                bytes: 2048,
                reason: "test".to_owned(),
                modified_at_unix: None,
                confidence: Confidence::Safe,
                approved_rule: None,
                custom_rule: None,
            }],
            review_candidates: Vec::new(),
            learning_observations: Vec::new(),
            workspaces: Vec::new(),
            warnings: Vec::new(),
            total_bytes: 2048,
            review_total_bytes: 0,
            observed_total_bytes: 0,
            protect_git_tracked: true,
        };

        record_scan(&report, Some(&database))?;
        let summary = summarize(30, Some(&database))?;
        let raw = std::fs::read(&database)?;

        assert_eq!(summary.scan_count, 1);
        assert_eq!(summary.latest_reclaimable_bytes, 2048);
        assert!(
            !raw.windows(b"private/node_modules".len())
                .any(|window| window == b"private/node_modules")
        );
        Ok(())
    }

    #[test]
    fn legacy_unversioned_database_should_migrate_without_losing_history() -> Result<()> {
        let temporary = tempdir()?;
        let database = temporary.path().join("history.sqlite3");
        let legacy = Connection::open(&database)?;
        legacy.execute_batch(
            "CREATE TABLE scan_events(id INTEGER PRIMARY KEY,at_unix INTEGER NOT NULL,total_bytes INTEGER NOT NULL,candidate_count INTEGER NOT NULL,categories_json TEXT NOT NULL);
             CREATE TABLE cleanup_events(id INTEGER PRIMARY KEY,at_unix INTEGER NOT NULL,removed_bytes INTEGER NOT NULL,quarantined_bytes INTEGER NOT NULL,removed_count INTEGER NOT NULL,held_count INTEGER NOT NULL,failures INTEGER NOT NULL);
             INSERT INTO scan_events(at_unix,total_bytes,candidate_count,categories_json) VALUES (0,4096,1,'{}');",
        )?;
        drop(legacy);

        let summary = summarize(u64::MAX / 86_400, Some(&database))?;
        let migrated = Connection::open(&database)?;

        assert_eq!(summary.scan_count, 1);
        assert_eq!(summary.latest_reclaimable_bytes, 4096);
        assert_eq!(schema_version(&migrated)?, CURRENT_SCHEMA_VERSION);
        Ok(())
    }

    #[test]
    fn newer_history_schema_should_be_rejected() -> Result<()> {
        let temporary = tempdir()?;
        let database = temporary.path().join("history.sqlite3");
        let connection = Connection::open(&database)?;
        connection.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)?;
        drop(connection);

        let error = summarize(30, Some(&database)).expect_err("newer schema must be rejected");

        assert!(error.to_string().contains("newer than supported"));
        Ok(())
    }

    #[test]
    fn history_should_count_privacy_safe_category_growth_events() -> Result<()> {
        let temporary = tempdir()?;
        let database = temporary.path().join("history.sqlite3");
        let connection = open(Some(&database))?;
        let first = serde_json::to_string(&BTreeMap::from([(Category::NodeModules, 1024)]))?;
        let second = serde_json::to_string(&BTreeMap::from([(Category::NodeModules, 2048)]))?;
        connection.execute(
            "INSERT INTO scan_events(at_unix,total_bytes,candidate_count,categories_json) VALUES (?1,1024,1,?2)",
            params![to_i64(now_unix()), first],
        )?;
        connection.execute(
            "INSERT INTO scan_events(at_unix,total_bytes,candidate_count,categories_json) VALUES (?1,2048,1,?2)",
            params![to_i64(now_unix()), second],
        )?;
        drop(connection);

        let summary = summarize(30, Some(&database))?;

        assert_eq!(summary.category_growth_events[&Category::NodeModules], 1);
        Ok(())
    }
}
