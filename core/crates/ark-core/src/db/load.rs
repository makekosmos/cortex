use crate::type_registry;

use super::objects::object_search_fts_exists;
use super::open::phase3_migration_completed;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::types::*;

use super::objects::{list_object_links, list_object_types, list_objects};
use super::sync::{bump_sync_version_vector, record_sync_tombstone};

include!("load/part1.rs");
include!("load/part2.rs");
include!("load/part3.rs");
