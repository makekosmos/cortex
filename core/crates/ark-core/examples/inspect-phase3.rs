//! Read-only migration diagnostics, with optional in-memory upgrade verification.
use rusqlite::{Connection, OpenFlags};

fn objects(conn: &Connection) -> rusqlite::Result<Vec<Vec<rusqlite::types::Value>>> {
    let mut statement = conn.prepare("SELECT * FROM objects ORDER BY id")?;
    let columns = statement.column_count();
    let rows = statement.query_map([], |row| (0..columns).map(|i| row.get(i)).collect())?;
    rows.collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("expected database path")?;
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let plan = ark_core::canonical_types::migration::plan_phase3(&conn)?;
    println!("status: {}", plan.status);
    for item in &plan.blocked {
        // No raw source values (task titles, notes, etc.) in diagnostics.
        println!("{}: {} at {}", item.source_kind, item.code, item.pointer);
    }
    if std::env::args().any(|arg| arg == "--check-upgrade") {
        if plan.status != "blocked" {
            return Err("upgrade probe expects a blocked legacy database".into());
        }
        let before = objects(&conn)?;
        let mut copy = Connection::open_in_memory()?;
        rusqlite::backup::Backup::new(&conn, &mut copy)?.run_to_completion(
            256,
            std::time::Duration::from_millis(1),
            None,
        )?;
        ark_core::db::init_schema(&copy)?;
        ark_core::db::init_schema(&copy)?;
        if objects(&copy)? != before {
            return Err("upgrade changed existing object rows".into());
        }
        for type_id in ["com.kosmos.task", "com.kosmos.project"] {
            if ark_core::type_registry::get_type(&copy, type_id, Some("1.1.0"))?.is_none() {
                return Err("write contract still missing after upgrade".into());
            }
        }
        println!("IN-MEMORY UPGRADE: PASS; existing objects unchanged; write contracts present");
    }
    Ok(())
}
