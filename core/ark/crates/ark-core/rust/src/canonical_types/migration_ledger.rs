//! Additive Phase 3 migration ledger and child-savepoint ownership.

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
    let mut rows: Vec<_> = records.iter().map(|r| json!({"sourceKind": r.source_kind.name(), "sourceId": r.source_id, "sourceHash": r.source_hash})).collect();
    rows.sort_by(|a, b| {
        (
            a["sourceKind"].as_str().unwrap(),
            a["sourceId"].as_str().unwrap(),
        )
            .cmp(&(
                b["sourceKind"].as_str().unwrap(),
                b["sourceId"].as_str().unwrap(),
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
fn load_items(conn: &Connection, contract: &str) -> Result<Vec<LedgerItem>, LedgerError> {
    let mut s=conn.prepare("SELECT contract_version,source_kind,source_id,source_hash,raw_source,status,canonical_hash,result_json,error_code,attempt,checkpoint,updated_at FROM canonical_migration_items WHERE contract_version=?1 ORDER BY source_kind,source_id").map_err(storage)?;
    let rows = s
        .query_map([contract], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get::<_, String>(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get::<_, String>(10)?,
                r.get(11)?,
            ))
        })
        .map_err(storage)?;
    rows.map(|row| {
        let (c, k, i, h, raw, st, ch, rj, ec, a, cp, u) = row.map_err(storage)?;
        Ok(LedgerItem {
            contract_version: c,
            source_kind: k,
            source_id: i,
            source_hash: h,
            raw_source: raw,
            status: parse_status(&st)?,
            canonical_hash: ch,
            result_json: rj,
            error_code: ec,
            attempt: a,
            checkpoint: parse_checkpoint(&cp)?,
            updated_at: u,
        })
    })
    .collect()
}

pub fn begin_or_resume(
    conn: &Connection,
    contract: &str,
    records: &[SourceRecord],
    now: &str,
) -> Result<MigrationRun, LedgerError> {
    ensure_ledger_schema(conn)?;
    let inv = inventory_hash(records);
    let existing: Option<(String,String,String,String,Option<String>)> = conn.query_row("SELECT source_inventory_hash,status,started_at,report_json,completed_at FROM canonical_migration_runs WHERE contract_version=?1", [contract], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(storage)?;
    if let Some((old, status, started, _, completed)) = existing {
        if old != inv {
            return Err(LedgerError::CanonicalConflict {
                source_kind: "inventory".into(),
                source_id: contract.into(),
            });
        }
        return Ok(MigrationRun {
            contract_version: contract.into(),
            source_inventory_hash: inv,
            status: if status == "completed" {
                MigrationRunStatus::Completed
            } else {
                MigrationRunStatus::Running
            },
            items: load_items(conn, contract)?,
            started_at: started,
            completed_at: completed,
        });
    }
    conn.execute("INSERT INTO canonical_migration_runs(contract_version,source_inventory_hash,status,started_at) VALUES(?1,?2,'running',?3)",params![contract,inv,now]).map_err(storage)?;
    for r in records {
        conn.execute("INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,raw_source,status,attempt,checkpoint,updated_at) VALUES(?1,?2,?3,?4,?5,'pending',0,'prepared',?6)",params![contract,r.source_kind.name(),r.source_id,r.source_hash,r.raw_source,now]).map_err(storage)?;
    }
    Ok(MigrationRun {
        contract_version: contract.into(),
        source_inventory_hash: inv,
        status: MigrationRunStatus::Running,
        items: load_items(conn, contract)?,
        started_at: now.into(),
        completed_at: None,
    })
}

/// Resume a completed run only after the caller revalidates every canonical projection.
pub fn begin_or_resume_with_validator<F>(
    conn: &Connection,
    contract: &str,
    records: &[SourceRecord],
    now: &str,
    validate: F,
) -> Result<MigrationRun, LedgerError>
where
    F: Fn(&LedgerItem, &SourceRecord) -> Result<(), LedgerError>,
{
    let run = begin_or_resume(conn, contract, records, now)?;
    if run.status == MigrationRunStatus::Completed {
        let by_key: std::collections::BTreeMap<_, _> = records
            .iter()
            .map(|record| {
                (
                    (record.source_kind.name(), record.source_id.as_str()),
                    record,
                )
            })
            .collect();
        for item in &run.items {
            let record = by_key
                .get(&(item.source_kind.as_str(), item.source_id.as_str()))
                .ok_or_else(|| LedgerError::CanonicalConflict {
                    source_kind: item.source_kind.clone(),
                    source_id: item.source_id.clone(),
                })?;
            if item.source_hash != record.source_hash {
                return Err(LedgerError::CanonicalConflict {
                    source_kind: item.source_kind.clone(),
                    source_id: item.source_id.clone(),
                });
            }
            validate(item, record)?;
        }
    }
    Ok(run)
}

/// A real retry is explicit; reading/resuming a pending item does not alter its attempt count.
pub fn retry_item(
    conn: &Connection,
    contract: &str,
    kind: &str,
    id: &str,
    now: &str,
) -> Result<i64, LedgerError> {
    let (status, attempt): (String, i64) = conn
        .query_row(
            "SELECT status,attempt FROM canonical_migration_items WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",
            params![contract, kind, id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(storage)?;
    if status != "pending" && status != "quarantined" {
        return Err(LedgerError::InvariantViolation(
            "terminal_regression".into(),
        ));
    }
    let next = attempt + 1;
    conn.execute(
        "UPDATE canonical_migration_items SET status='pending',checkpoint='prepared',error_code=NULL,attempt=?4,updated_at=?5 WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",
        params![contract, kind, id, next, now],
    )
    .map_err(storage)?;
    Ok(next)
}

pub struct ChildSavepoint<'a> {
    conn: &'a Connection,
    name: String,
    finished: bool,
}
impl<'a> ChildSavepoint<'a> {
    pub fn begin(conn: &'a Connection, item: &LedgerItem) -> Result<Self, LedgerError> {
        let name = format!(
            "ledger_item_{}_{}",
            item.source_kind
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_"),
            item.source_id
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        );
        conn.execute_batch(&format!("SAVEPOINT {name}"))
            .map_err(storage)?;
        Ok(Self {
            conn,
            name,
            finished: false,
        })
    }
    pub fn commit(&mut self) -> Result<(), LedgerError> {
        if self.finished {
            return Err(LedgerError::InvariantViolation("savepoint_finished".into()));
        }
        self.conn
            .execute_batch(&format!("RELEASE SAVEPOINT {}", self.name))
            .map_err(storage)?;
        self.finished = true;
        Ok(())
    }
    pub fn rollback(&mut self, _error: LedgerError) -> Result<(), LedgerError> {
        if self.finished {
            return Err(LedgerError::InvariantViolation("savepoint_finished".into()));
        }
        self.conn
            .execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {}; RELEASE SAVEPOINT {}",
                self.name, self.name
            ))
            .map_err(storage)?;
        self.finished = true;
        Ok(())
    }
}
impl Drop for ChildSavepoint<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT {}; RELEASE SAVEPOINT {}",
                self.name, self.name
            ));
        }
    }
}

pub fn transition_item(
    conn: &Connection,
    contract: &str,
    kind: &str,
    id: &str,
    status: ItemStatus,
    checkpoint: ItemCheckpoint,
    canonical_hash: Option<&str>,
    result_json: &str,
    error_code: Option<&str>,
    now: &str,
) -> Result<(), LedgerError> {
    let current: (String,String,i64)=conn.query_row("SELECT status,checkpoint,attempt FROM canonical_migration_items WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",params![contract,kind,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(storage)?;
    if current.0 != "pending" && current.1 == "committed" {
        return Err(LedgerError::InvariantViolation(
            "terminal_regression".into(),
        ));
    }
    if checkpoint == ItemCheckpoint::Committed && matches!(status, ItemStatus::Pending) {
        return Err(LedgerError::InvariantViolation("illegal_transition".into()));
    }
    conn.execute("UPDATE canonical_migration_items SET status=?4,checkpoint=?5,canonical_hash=?6,result_json=?7,error_code=?8,attempt=?9,updated_at=?10 WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3",params![contract,kind,id,status.as_str(),checkpoint.as_str(),canonical_hash,result_json,error_code,current.2+if current.0=="pending"{1}else{0},now]).map_err(storage)?;
    Ok(())
}

pub fn complete_run(
    conn: &Connection,
    contract: &str,
    report_json: &str,
    now: &str,
) -> Result<(), LedgerError> {
    let pending:i64=conn.query_row("SELECT COUNT(*) FROM canonical_migration_items WHERE contract_version=?1 AND checkpoint!='committed'",[contract],|r|r.get(0)).map_err(storage)?;
    if pending != 0 {
        return Err(LedgerError::InvariantViolation(
            "items_not_committed".into(),
        ));
    }
    conn.execute("UPDATE canonical_migration_runs SET status='completed',report_json=?2,completed_at=?3 WHERE contract_version=?1",params![contract,report_json,now]).map_err(storage)?;
    Ok(())
}

pub fn canonical_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
