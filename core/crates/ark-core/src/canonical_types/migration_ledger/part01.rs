// Additive Phase 3 migration ledger and child-savepoint ownership.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::preflight::SourceRecord;

pub const CONTRACT_VERSION: &str = "phase3-canonical-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MigrationRunStatus {
    Running,
    Completed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    Pending,
    Migrated,
    Unchanged,
    Quarantined,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemCheckpoint {
    Prepared,
    Applied,
    Committed,
}

impl ItemStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Migrated => "migrated",
            Self::Unchanged => "unchanged",
            Self::Quarantined => "quarantined",
        }
    }
}
impl ItemCheckpoint {
    fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Applied => "applied",
            Self::Committed => "committed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerItem {
    pub contract_version: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_hash: String,
    pub raw_source: Vec<u8>,
    pub status: ItemStatus,
    pub canonical_hash: Option<String>,
    pub result_json: String,
    pub error_code: Option<String>,
    pub attempt: i64,
    pub checkpoint: ItemCheckpoint,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRun {
    pub contract_version: String,
    pub source_inventory_hash: String,
    pub status: MigrationRunStatus,
    pub items: Vec<LedgerItem>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    Storage(String),
    InvariantViolation(String),
    CanonicalConflict {
        source_kind: String,
        source_id: String,
    },
}
impl LedgerError {
    pub fn invariant(message: impl Into<String>) -> Self {
        Self::InvariantViolation(message.into())
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "Storage",
            Self::InvariantViolation(_) => "InvariantViolation",
            Self::CanonicalConflict { .. } => "CanonicalConflict",
        }
    }
}
impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}
impl std::error::Error for LedgerError {}
fn storage(e: impl std::fmt::Display) -> LedgerError {
    LedgerError::Storage(e.to_string())
}

const TABLES: &[(&str, &str)] = &[
    ("canonical_migration_runs", "CREATE TABLE canonical_migration_runs (contract_version TEXT PRIMARY KEY, source_inventory_hash TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('running','completed')), report_json TEXT NOT NULL DEFAULT '{}', started_at TEXT NOT NULL, completed_at TEXT)"),
    ("canonical_migration_items", "CREATE TABLE canonical_migration_items (contract_version TEXT NOT NULL, source_kind TEXT NOT NULL, source_id TEXT NOT NULL, source_hash TEXT NOT NULL, raw_source BLOB NOT NULL DEFAULT X'', status TEXT NOT NULL CHECK(status IN ('pending','migrated','unchanged','quarantined')), canonical_hash TEXT, result_json TEXT NOT NULL DEFAULT '{}', error_code TEXT, attempt INTEGER NOT NULL DEFAULT 0 CHECK(attempt >= 0), checkpoint TEXT NOT NULL DEFAULT 'committed' CHECK(checkpoint IN ('prepared','applied','committed')), updated_at TEXT NOT NULL, PRIMARY KEY(contract_version, source_kind, source_id), FOREIGN KEY(contract_version) REFERENCES canonical_migration_runs(contract_version) ON DELETE CASCADE)"),
];

fn table_sql(conn: &Connection, table: &str) -> Result<Option<String>, LedgerError> {
    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |r| r.get(0),
    )
    .optional()
    .map_err(storage)
}
fn columns(conn: &Connection, table: &str) -> Result<Vec<(String, i64, String, i64)>, LedgerError> {
    let mut s = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(storage)?;
    let result = s
        .query_map([], |r| {
            Ok((r.get(1)?, r.get(5)?, r.get::<_, String>(2)?, r.get(3)?))
        })
        .map_err(storage)?
        .collect::<Result<_, _>>()
        .map_err(storage);
    result
}
fn validate_existing(conn: &Connection, table: &str, expected: &str) -> Result<(), LedgerError> {
    let sql = table_sql(conn, table)?
        .ok_or_else(|| LedgerError::InvariantViolation(format!("missing:{table}")))?;
    let actual = columns(conn, table)?;
    let exp = if table == "canonical_migration_runs" {
        vec![
            ("contract_version".into(), 1, "TEXT".into(), 0),
            ("source_inventory_hash".into(), 0, "TEXT".into(), 1),
            ("status".into(), 0, "TEXT".into(), 1),
            ("report_json".into(), 0, "TEXT".into(), 1),
            ("started_at".into(), 0, "TEXT".into(), 1),
            ("completed_at".into(), 0, "TEXT".into(), 0),
        ]
    } else {
        vec![
            ("contract_version".into(), 1, "TEXT".into(), 1),
            ("source_kind".into(), 2, "TEXT".into(), 1),
            ("source_id".into(), 3, "TEXT".into(), 1),
            ("source_hash".into(), 0, "TEXT".into(), 1),
            ("raw_source".into(), 0, "BLOB".into(), 1),
            ("status".into(), 0, "TEXT".into(), 1),
            ("canonical_hash".into(), 0, "TEXT".into(), 0),
            ("result_json".into(), 0, "TEXT".into(), 1),
            ("error_code".into(), 0, "TEXT".into(), 0),
            ("attempt".into(), 0, "INTEGER".into(), 1),
            ("checkpoint".into(), 0, "TEXT".into(), 1),
            ("updated_at".into(), 0, "TEXT".into(), 1),
        ]
    };
    if actual != exp
        || !sql.to_ascii_lowercase().contains("check")
        || !sql.to_ascii_lowercase().contains("primary key")
    {
        return Err(LedgerError::InvariantViolation(format!(
            "schema_parity:{table}"
        )));
    }
    let _ = expected;
    Ok(())
}

/// Create the two additive ledger tables, or validate them without ALTER/repair.
pub fn ensure_ledger_schema(conn: &Connection) -> Result<(), LedgerError> {
    conn.execute_batch("SAVEPOINT ledger_schema")
        .map_err(storage)?;
    let result = (|| {
        for (name, ddl) in TABLES {
            if table_sql(conn, name)?.is_some() {
                validate_existing(conn, name, ddl)?;
            } else {
                conn.execute_batch(ddl).map_err(storage)?;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ledger_schema")
            .map_err(storage),
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ledger_schema; RELEASE SAVEPOINT ledger_schema",
            );
            Err(e)
        }
    }
}

fn inventory_hash(records: &[SourceRecord]) -> String {
    let mut rows: Vec<_> = records
        .iter()
        .map(|record| {
            json!({
                "sourceKind": record.source_kind.name(),
                "sourceId": record.source_id,
                "sourceHash": record.source_hash,
            })
        })
        .collect();
    rows.sort_by(|left, right| {
        (
            left["sourceKind"].as_str().unwrap_or_default(),
            left["sourceId"].as_str().unwrap_or_default(),
        )
            .cmp(&(
                right["sourceKind"].as_str().unwrap_or_default(),
                right["sourceId"].as_str().unwrap_or_default(),
            ))
    });
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&rows).expect("JSON"))
    )
}
fn parse_status(v: &str) -> Result<ItemStatus, LedgerError> {
    match v {
        "pending" => Ok(ItemStatus::Pending),
        "migrated" => Ok(ItemStatus::Migrated),
        "unchanged" => Ok(ItemStatus::Unchanged),
        "quarantined" => Ok(ItemStatus::Quarantined),
        _ => Err(LedgerError::InvariantViolation("item_status".into())),
    }
}
fn parse_checkpoint(v: &str) -> Result<ItemCheckpoint, LedgerError> {
    match v {
        "prepared" => Ok(ItemCheckpoint::Prepared),
        "applied" => Ok(ItemCheckpoint::Applied),
        "committed" => Ok(ItemCheckpoint::Committed),
        _ => Err(LedgerError::InvariantViolation("item_checkpoint".into())),
    }
}
