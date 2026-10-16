use chrono::{Duration, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::types::*;

include!("usage/part1a.rs");
include!("usage/part1b.rs");
include!("usage/part2a.rs");
include!("usage/part2b.rs");
include!("usage/part2c.rs");
include!("usage/part3a.rs");
include!("usage/part3b.rs");
