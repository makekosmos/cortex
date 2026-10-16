use crate::schema::CREATE_OBJECT_SEARCH_FTS;
use crate::type_registry;
use serde::Serialize;

use super::open::replay_pending_for_type;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Value};

use crate::types::*;

include!("objects/part1.rs");
include!("objects/part2a.rs");
include!("objects/part2b.rs");
include!("objects/part3a.rs");
include!("objects/part3b.rs");
include!("objects/part4.rs");
