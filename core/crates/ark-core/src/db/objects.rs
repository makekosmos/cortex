use super::open::replay_pending_for_type;
use crate::type_registry;
pub(in crate::db) use ark_store::objects::list_other_object_link_ids;
pub use ark_store::objects::{
    delete_object_link, ensure_object_search_fts, get_object, get_objects_by_ids,
    list_object_links, list_object_summaries, list_object_summaries_by_type, list_objects,
    list_objects_by_type, list_running_time_entries, object_search_fts_exists,
    rebuild_object_search_fts, search_objects, upsert_object_link, SearchResult,
};
use rusqlite::{params, Connection, OptionalExtension};

pub(in crate::db) fn rollback_savepoint(conn: &Connection, name: &str) {
    let _ = conn.execute_batch(&format!(
        "ROLLBACK TO SAVEPOINT {name}; RELEASE SAVEPOINT {name};"
    ));
}

use crate::types::*;

include!("objects/types.rs");
include!("objects/records.rs");
