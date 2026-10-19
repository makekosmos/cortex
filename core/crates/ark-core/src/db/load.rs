use crate::type_registry;

use super::objects::object_search_fts_exists;
use super::open::phase3_migration_completed;

use rusqlite::Connection;

use crate::types::*;

use super::objects::{list_object_links, list_object_types, list_objects};

include!("load/all_data.rs");
