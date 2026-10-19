use chrono::Utc;
use rusqlite::{params, Connection};

use crate::hlc::HLC;
use crate::types::*;

use super::open::{
    is_sequenced_usage_entity, usage_cursor_key, USAGE_SEQUENCE_MIGRATION_KEY, VERSION_VECTOR_KEY,
};

include!("sync/tombstones_and_usage_sequence.rs");
