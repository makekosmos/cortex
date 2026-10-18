use crate::type_registry;
use ark_store::schema::CREATE_OBJECT_SEARCH_FTS;
use serde::Serialize;

use super::open::replay_pending_for_type;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

use crate::types::*;

include!("objects/types.rs");
include!("objects/records.rs");
include!("objects/queries.rs");
include!("objects/search.rs");
include!("objects/search_text.rs");
include!("objects/links.rs");
