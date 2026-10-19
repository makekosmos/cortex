use super::open::replay_pending_for_type;
use crate::type_registry;
pub use ark_store::objects::{
    ensure_object_search_fts, get_object, get_objects_by_ids, list_object_summaries,
    list_object_summaries_by_type, list_objects, list_objects_by_type, list_running_time_entries,
    object_search_fts_exists, rebuild_object_search_fts, search_objects, SearchResult,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::types::*;

include!("objects/types.rs");
include!("objects/records.rs");
include!("objects/links.rs");
