mod fixtures;
mod todo_project_and_usage;
mod todo_tags_and_checklists;
mod usage_analytics_dedup;
mod usage_queries_and_objects;

use chrono::Utc;
use fixtures::*;
use rusqlite::{params, Connection};
use serde_json::json;

use crate::legacy_entities::{
    batch_upsert_todos, delete_project, delete_todo, upsert_project, upsert_tag, upsert_todo,
};
use crate::objects::{
    delete_object_link, get_object, get_objects_by_ids, list_object_links, list_objects_by_type,
    upsert_object_link,
};
use crate::usage_queries::merge_top_apps;
use ark_domain::types::*;
