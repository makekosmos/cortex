use ark_core::types::ObjectType;
use ark_store::type_registry::upsert_object_type;
use rusqlite::Connection;

fn setup() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    ark_store::open::init_schema(&conn).unwrap();
    conn
}
