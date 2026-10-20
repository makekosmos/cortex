use rusqlite::{Connection, Result, Transaction};

#[derive(Debug, Clone, Copy)]
pub enum ActorLogOrder {
    Insertion,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorLogEntry {
    pub sequence: i64,
    pub source: String,
}

pub struct ActorTestLog;

impl ActorTestLog {
    pub fn create(connection: &Connection) -> Result<()> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS actor_test_log (
                sequence INTEGER NOT NULL,
                source TEXT NOT NULL
            )",
        )
    }

    pub fn append(transaction: &Transaction<'_>, sequence: i64, source: &str) -> Result<()> {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS actor_test_log (
                sequence INTEGER NOT NULL,
                source TEXT NOT NULL
            )",
        )?;
        transaction.execute(
            "INSERT INTO actor_test_log(sequence, source) VALUES (?1, ?2)",
            rusqlite::params![sequence, source],
        )?;
        Ok(())
    }

    pub fn entries(connection: &Connection, order: ActorLogOrder) -> Result<Vec<ActorLogEntry>> {
        let sql = match order {
            ActorLogOrder::Insertion => {
                "SELECT sequence, source FROM actor_test_log ORDER BY rowid"
            }
            ActorLogOrder::Sequence => {
                "SELECT sequence, source FROM actor_test_log ORDER BY sequence"
            }
        };
        let mut statement = connection.prepare(sql)?;
        let entries = statement
            .query_map([], |row| {
                Ok(ActorLogEntry {
                    sequence: row.get(0)?,
                    source: row.get(1)?,
                })
            })?
            .collect();
        entries
    }

    pub fn count(connection: &Connection) -> Result<i64> {
        connection.query_row("SELECT COUNT(*) FROM actor_test_log", [], |row| row.get(0))
    }

    pub fn contains(connection: &Connection, sequence: i64) -> Result<bool> {
        connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM actor_test_log WHERE sequence = ?1)",
            [sequence],
            |row| row.get(0),
        )
    }

    pub fn panic_probe_exists(connection: &Connection) -> Result<bool> {
        connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = 'panic_probe'
            )",
            [],
            |row| row.get(0),
        )
    }

    pub fn insert_panic_probe(transaction: &Transaction<'_>) -> Result<()> {
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS panic_probe(value INTEGER NOT NULL);
             INSERT INTO panic_probe VALUES (1);",
        )
    }
}
