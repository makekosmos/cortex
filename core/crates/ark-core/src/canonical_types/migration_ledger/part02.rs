fn load_items(conn: &Connection, contract: &str) -> Result<Vec<LedgerItem>, LedgerError> {
    let mut s=conn.prepare(concat!("SELECT contract_version,source_kind,source_id,source_hash,raw_source,status,","canonical_hash,result_json,error_code,attempt,checkpoint,updated_at FROM ","canonical_migration_items WHERE contract_version=?1 ORDER BY source_kind,","source_id")).map_err(storage)?;
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
    let existing: Option<(String,String,String,String,Option<String>)> = conn.query_row(concat!("SELECT source_inventory_hash,status,started_at,report_json,completed_at ","FROM canonical_migration_runs WHERE contract_version=?1"), [contract], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(storage)?;
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
    conn.execute(concat!("INSERT INTO canonical_migration_runs(contract_version,source_inventory_hash,","status,started_at) VALUES(?1,?2,'running',?3)"),params![contract,inv,now]).map_err(storage)?;
    for r in records {
        conn.execute(concat!("INSERT INTO canonical_migration_items(contract_version,source_kind,","source_id,source_hash,raw_source,status,attempt,checkpoint,updated_at) ","VALUES(?1,?2,?3,?4,?5,'pending',0,'prepared',?6)"),params![contract,r.source_kind.name(),r.source_id,r.source_hash,r.raw_source,now]).map_err(storage)?;
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
            concat!("SELECT status,attempt FROM canonical_migration_items WHERE ","contract_version=?1 AND source_kind=?2 AND source_id=?3"),
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
        concat!("UPDATE canonical_migration_items SET status='pending',checkpoint='prepared',","error_code=NULL,attempt=?4,updated_at=?5 WHERE contract_version=?1 AND ","source_kind=?2 AND source_id=?3"),
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

/// Fields written to one `canonical_migration_items` row: the row's columns
/// plus the write timestamp.
pub struct ItemTransition<'a> {
    pub contract: &'a str,
    pub kind: &'a str,
    pub id: &'a str,
    pub status: ItemStatus,
    pub checkpoint: ItemCheckpoint,
    pub canonical_hash: Option<&'a str>,
    pub result_json: &'a str,
    pub error_code: Option<&'a str>,
    pub now: &'a str,
}

// Applies one ledger row.
pub fn transition_item(conn: &Connection, t: ItemTransition<'_>) -> Result<(), LedgerError> {
    let ItemTransition {
        contract,
        kind,
        id,
        status,
        checkpoint,
        canonical_hash,
        result_json,
        error_code,
        now,
    } = t;
    let current: (String,String,i64)=conn.query_row(concat!("SELECT status,checkpoint,attempt FROM canonical_migration_items WHERE ","contract_version=?1 AND source_kind=?2 AND source_id=?3"),params![contract,kind,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(storage)?;
    if current.0 != "pending" && current.1 == "committed" {
        return Err(LedgerError::InvariantViolation(
            "terminal_regression".into(),
        ));
    }
    if checkpoint == ItemCheckpoint::Committed && matches!(status, ItemStatus::Pending) {
        return Err(LedgerError::InvariantViolation("illegal_transition".into()));
    }
    conn.execute(concat!("UPDATE canonical_migration_items SET status=?4,checkpoint=?5,","canonical_hash=?6,result_json=?7,error_code=?8,attempt=?9,updated_at=?10 ","WHERE contract_version=?1 AND source_kind=?2 AND source_id=?3"),params![contract,kind,id,status.as_str(),checkpoint.as_str(),canonical_hash,result_json,error_code,current.2+if current.0=="pending"{1}else{0},now]).map_err(storage)?;
    Ok(())
}

pub fn complete_run(
    conn: &Connection,
    contract: &str,
    report_json: &str,
    now: &str,
) -> Result<(), LedgerError> {
    let pending:i64=conn.query_row(concat!("SELECT COUNT(*) FROM canonical_migration_items WHERE contract_version=?1 ","AND checkpoint!='committed'"),[contract],|r|r.get(0)).map_err(storage)?;
    if pending != 0 {
        return Err(LedgerError::InvariantViolation(
            "items_not_committed".into(),
        ));
    }
    conn.execute(concat!("UPDATE canonical_migration_runs SET status='completed',report_json=?2,","completed_at=?3 WHERE contract_version=?1"),params![contract,report_json,now]).map_err(storage)?;
    Ok(())
}

pub fn canonical_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
