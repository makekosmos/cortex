use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::types::*;

include!("usage/records.rs");
include!("usage/spans_and_title_totals.rs");
include!("usage/analytics.rs");
include!("usage/process_queries.rs");
include!("usage/process_projection.rs");
include!("usage/game_playtime_queries.rs");
include!("usage/game_playtime_summary.rs");
