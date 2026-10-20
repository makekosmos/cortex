use std::path::Path;

use rusqlite::{Connection, Result, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorFailureProbeState {
    pub mutations: i64,
    pub receipts: i64,
    pub deferred_children: i64,
}

pub struct ProductionActorFailureProbe;

pub struct CompetingWriter {
    connection: Connection,
}

impl CompetingWriter {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            connection: Connection::open(path)?,
        })
    }

    pub fn begin_immediate(&self) -> Result<()> {
        self.connection.execute_batch("BEGIN IMMEDIATE")
    }

    pub fn rollback(&self) -> Result<()> {
        self.connection.execute_batch("ROLLBACK")
    }
}

impl ProductionActorFailureProbe {
    pub fn create_restore_snapshot_fixture(
        path: &Path,
        snapshot_path: &Path,
    ) -> Result<(), String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "CREATE TABLE restore_failure_probe(value TEXT NOT NULL);
                 INSERT INTO restore_failure_probe(value) VALUES ('pre-restore');",
            )
            .map_err(|error| error.to_string())?;
        let snapshot_path = snapshot_path
            .to_str()
            .ok_or_else(|| "snapshot fixture path is not valid UTF-8".to_string())?;
        crate::snapshot::backup_connection_to_file(&connection, snapshot_path)
    }

    pub fn restore_probe_value(path: &Path) -> Result<String> {
        let connection = Connection::open(path)?;
        connection.query_row("SELECT value FROM restore_failure_probe", [], |row| {
            row.get(0)
        })
    }

    pub fn prepare(connection: &Connection) -> Result<()> {
        connection.execute_batch(
            "CREATE TABLE production_actor_failure_probe(payload BLOB NOT NULL);
             CREATE TABLE production_actor_parent(id INTEGER PRIMARY KEY);
                 CREATE TABLE production_actor_deferred_child(
                     parent_id INTEGER NOT NULL,
                     FOREIGN KEY(parent_id) REFERENCES production_actor_parent(id)
                         DEFERRABLE INITIALLY DEFERRED
                 );",
        )
    }

    pub fn insert_mutation(transaction: &Transaction<'_>, payload_bytes: i64) -> Result<()> {
        transaction.execute(
            "INSERT INTO production_actor_failure_probe(payload) VALUES (zeroblob(?1))",
            [payload_bytes],
        )?;
        Ok(())
    }

    pub fn trigger_deferred_commit_failure(transaction: &Transaction<'_>) -> Result<()> {
        transaction.execute(
            "INSERT INTO production_actor_failure_probe(payload) VALUES (zeroblob(0))",
            [],
        )?;
        transaction.execute(
            "INSERT INTO production_actor_deferred_child(parent_id) VALUES (99)",
            [],
        )?;
        Ok(())
    }

    pub fn inspect(connection: &Connection, operation_id: &str) -> Result<ActorFailureProbeState> {
        let mutations = connection.query_row(
            "SELECT COUNT(*) FROM production_actor_failure_probe",
            [],
            |row| row.get(0),
        )?;
        let receipts = connection.query_row(
            "SELECT COUNT(*) FROM ark_operation_receipts WHERE operation_id = ?1",
            [operation_id],
            |row| row.get(0),
        )?;
        let deferred_children = connection.query_row(
            "SELECT COUNT(*) FROM production_actor_deferred_child",
            [],
            |row| row.get(0),
        )?;
        Ok(ActorFailureProbeState {
            mutations,
            receipts,
            deferred_children,
        })
    }

    pub fn inspect_path(path: &Path, operation_id: &str) -> Result<ActorFailureProbeState> {
        let connection = Connection::open(path)?;
        Self::inspect(&connection, operation_id)
    }

    pub fn page_count(connection: &Connection) -> Result<u32> {
        connection.query_row("PRAGMA page_count", [], |row| row.get(0))
    }

    pub fn set_max_page_count(connection: &Connection, page_count: u32) -> Result<u32> {
        connection.pragma_update(None, "max_page_count", page_count)?;
        connection.query_row("PRAGMA max_page_count", [], |row| row.get(0))
    }
}
