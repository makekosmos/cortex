use super::open::replay_pending_for_type;
use crate::type_registry;
use rusqlite::Connection;

pub(in crate::db) fn rollback_savepoint(conn: &Connection, name: &str) {
    let _ = conn.execute_batch(&format!(
        "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
    ));
}

use crate::types::*;

include!("objects/types.rs");
