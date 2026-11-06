use std::sync::Arc;

use super::*;
use crate::hlc::HLC;
use crate::integration_replication::*;
use crate::types::*;
use ark_domain::integration_export::export_integration_replication;
use ark_domain::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};
use serde_json::{json, Value};

#[path = "test_fixtures.rs"]
mod test_fixtures;
#[path = "tests_integration_replication.rs"]
mod tests_integration_replication;
#[path = "tests_integration_replication_adapter.rs"]
mod tests_integration_replication_adapter;
#[path = "tests_integration_replication_apply.rs"]
mod tests_integration_replication_apply;
#[path = "tests_integration_replication_export.rs"]
mod tests_integration_replication_export;
#[path = "tests_integration_replication_issuer_lookup.rs"]
mod tests_integration_replication_issuer_lookup;
#[path = "tests_integration_replication_issuer_publish_lookup.rs"]
mod tests_integration_replication_issuer_publish_lookup;
#[path = "tests_integration_replication_security.rs"]
mod tests_integration_replication_security;
#[path = "tests_sync.rs"]
mod tests_sync;
