use super::{SqliteWriter, StoreTransactionFailure};
use crate::committed_feed::InboxState;
use ark_application::sync_storage::{
    InboundApplyOutcome, InboxReceiptWrite, InboxReceiptWriteError, ReplayNotice,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedInboundEntityOutcome {
    pub state: InboxState,
    pub outcome: InboundApplyOutcome,
    pub replay_notices: Vec<ReplayNotice>,
    pub pending_type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticatedInboundEntityError {
    Receipt(InboxReceiptWriteError),
    Mutation(String),
}

impl SqliteWriter {
    /// Receives and applies one authenticated non-core.task entity in a single
    /// immediate transaction, including its committed-feed append.
    pub fn apply_authenticated_inbound_entity(
        &mut self,
        write: InboxReceiptWrite,
        entity: ark_domain::types::SyncEntity,
    ) -> Result<
        AuthenticatedInboundEntityOutcome,
        StoreTransactionFailure<AuthenticatedInboundEntityError>,
    > {
        self.write_with_capabilities(|capabilities| {
            apply_inbound_entity(capabilities.transaction, write, entity)
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_authenticated_inbound_entity_on_connection(
        connection: &mut super::Connection,
        write: InboxReceiptWrite,
        entity: ark_domain::types::SyncEntity,
    ) -> Result<
        AuthenticatedInboundEntityOutcome,
        super::TransactionFailure<AuthenticatedInboundEntityError>,
    > {
        Self::immediate_on_connection(connection, |transaction| {
            apply_inbound_entity(transaction, write, entity)
        })
    }
}

fn apply_inbound_entity(
    transaction: &super::Transaction<'_>,
    write: InboxReceiptWrite,
    entity: ark_domain::types::SyncEntity,
) -> Result<AuthenticatedInboundEntityOutcome, AuthenticatedInboundEntityError> {
    if write.payload_json.len() > ark_application::sync_storage::MAX_SYNC_BATCH_BYTES {
        return Err(AuthenticatedInboundEntityError::Receipt(
            InboxReceiptWriteError::Inbox("sync_inbox_payload_limit_exceeded".into()),
        ));
    }
    if let Some(policy) = &write.policy {
        let profile =
            crate::data_platform::load_sync_profile_for_sync(transaction, &policy.local_device_id)
                .map_err(|error| {
                    AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::PolicyStorage(
                        error,
                    ))
                })?;
        if ark_application::sync_policy::sync_profile_identity(profile.as_ref())
            != policy.expected_identity
        {
            return Err(AuthenticatedInboundEntityError::Receipt(
                InboxReceiptWriteError::PolicyChanged,
            ));
        }
    }
    let admitted = crate::device_registry::is_device_admitted(transaction, &write.peer_device_id)
        .map_err(|error| {
            AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::RegistryStorage(error))
        })?
        .unwrap_or(false);
    if !admitted {
        return Err(AuthenticatedInboundEntityError::Receipt(
            InboxReceiptWriteError::PeerNotAdmitted,
        ));
    }
    let removed = crate::sync_kv::get_sync_kv(transaction, "sync.removed_peers")
        .map_err(|error| {
            AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::RemovedPeersStorage(
                error,
            ))
        })?
        .map(|raw| serde_json::from_str::<Vec<String>>(&raw))
        .transpose()
        .map_err(|error| {
            AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::RemovedPeersStorage(
                error.to_string(),
            ))
        })?
        .unwrap_or_default();
    if removed.iter().any(|peer| peer == &write.peer_device_id) {
        return Err(AuthenticatedInboundEntityError::Receipt(
            InboxReceiptWriteError::PeerRemoved,
        ));
    }
    let state = crate::committed_feed::receive_inbox_payload(
        transaction,
        &write.peer_device_id,
        &write.message_id,
        &write.payload_json,
    )
    .map_err(|error| {
        AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::Inbox(error))
    })?;
    if state == InboxState::Applied || state == InboxState::Pending {
        return Ok(AuthenticatedInboundEntityOutcome {
            state,
            outcome: if state == InboxState::Applied {
                InboundApplyOutcome::Applied
            } else {
                InboundApplyOutcome::Pending
            },
            replay_notices: Vec::new(),
            pending_type_id: None,
        });
    }
    if state != InboxState::Received {
        return Err(AuthenticatedInboundEntityError::Receipt(
            InboxReceiptWriteError::Inbox("sync_inbox_payload_not_ready".into()),
        ));
    }
    if serde_json::to_string(&entity)
        .map_err(|error| AuthenticatedInboundEntityError::Mutation(error.to_string()))?
        != write.payload_json
    {
        return Err(AuthenticatedInboundEntityError::Receipt(
            InboxReceiptWriteError::Inbox("sync_inbox_payload_mismatch".into()),
        ));
    }
    let (outcome, replay_notices) =
        crate::sync_entities::apply_inbound_entity(transaction, &entity)
            .map_err(AuthenticatedInboundEntityError::Mutation)?;
    let state = match outcome {
        InboundApplyOutcome::Applied | InboundApplyOutcome::Stale => InboxState::Applied,
        InboundApplyOutcome::Pending => InboxState::Pending,
    };
    crate::committed_feed::set_inbox_state(
        transaction,
        &write.peer_device_id,
        &write.message_id,
        state,
    )
    .map_err(|error| {
        AuthenticatedInboundEntityError::Receipt(InboxReceiptWriteError::Inbox(error))
    })?;
    let pending_type_id = (outcome == InboundApplyOutcome::Pending)
        .then(|| {
            entity
                .data
                .get("typeId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .flatten();
    Ok(AuthenticatedInboundEntityOutcome {
        state,
        outcome,
        replay_notices,
        pending_type_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{AuthenticatedInboundEntityError, AuthenticatedInboundEntityOutcome};
    use crate::committed_feed::InboxState;
    use crate::writer::{SqliteWriter, TransactionFailure};
    use ark_application::sync_storage::{InboundApplyOutcome, InboxReceiptWrite};
    use ark_domain::types::SyncEntity;
    use rusqlite::Connection;
    use serde_json::{Map, Value};

    fn setup() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("schema initializes");
        crate::type_registry::migrate_schema(&connection)
            .expect("type registry schema initializes");
        connection
            .execute(
                "INSERT INTO device_registry_projection(
                    device_id, signing_public_key, key_epoch, admission_epoch
                 ) VALUES ('peer', zeroblob(32), 1, 1)",
                [],
            )
            .expect("peer admitted");
        connection
    }

    fn entity(entity_type: &str, id: &str, data: Value, hlc: &str, seq: u64) -> SyncEntity {
        SyncEntity {
            entity_type: entity_type.into(),
            id: id.into(),
            data: data.as_object().cloned().unwrap_or_default(),
            hlc: hlc.into(),
            deleted: None,
            origin_device_id: Some("peer".into()),
            origin_seq: Some(seq),
        }
    }

    fn write(message_id: &str, entity: &SyncEntity) -> InboxReceiptWrite {
        InboxReceiptWrite::new(
            "peer".into(),
            message_id.into(),
            serde_json::to_string(entity).expect("payload serializes"),
        )
    }

    fn usage_day(hlc: &str, seq: u64) -> SyncEntity {
        entity(
            "usage_day",
            "day-1",
            serde_json::json!({
                "deviceId":"peer", "day":"2026-10-20", "updatedAt":hlc,
                "payloadJson":{"a":[],"t":[],"s":[]}
            }),
            hlc,
            seq,
        )
    }

    fn count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count reads")
    }

    #[test]
    fn authenticated_entity_applies_once_and_rejects_receipt_payload_mismatch() {
        let mut connection = setup();
        let entity = usage_day("2026-10-20T00:00:00.000Z:000001:peer", 1);
        let first = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write("message", &entity),
            entity.clone(),
        )
        .expect("entity applies");
        assert_eq!(first.state, InboxState::Applied);
        assert_eq!(first.outcome, InboundApplyOutcome::Applied);
        assert_eq!(count(&connection, "ark_committed_feed"), 1);
        let duplicate = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write("message", &entity),
            entity.clone(),
        )
        .expect("duplicate is idempotent");
        assert_eq!(duplicate.state, InboxState::Applied);
        assert_eq!(count(&connection, "ark_committed_feed"), 1);
        let mismatch = usage_day("2026-10-20T00:00:01.000Z:000001:peer", 2);
        let failure = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write("message", &mismatch),
            mismatch,
        )
        .expect_err("same receipt key cannot change payload");
        assert!(matches!(
            failure,
            TransactionFailure::Operation(AuthenticatedInboundEntityError::Receipt(_))
        ));
        assert_eq!(count(&connection, "ark_committed_feed"), 1);
    }

    #[test]
    fn authenticated_entity_records_stale_feed_and_rolls_back_failed_mutation() {
        let mut connection = setup();
        let stale = usage_day("2026-10-20T00:00:00.000Z:000001:peer", 1);
        connection
            .execute(
                "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
                 VALUES ('usage_day', 'day-1', ?1)",
                [&stale.hlc],
            )
            .expect("existing revision inserted");
        let result = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write("stale", &stale),
            stale,
        )
        .expect("stale receipt persists");
        assert_eq!(result.outcome, InboundApplyOutcome::Stale);
        assert_eq!(count(&connection, "ark_committed_feed"), 1);
        assert_eq!(count(&connection, "ark_sync_inbox"), 1);

        let invalid = entity("unknown_kind", "bad", Value::Object(Map::new()), "r2", 2);
        let failure = SqliteWriter::apply_authenticated_inbound_entity_on_connection(
            &mut connection,
            write("rollback", &invalid),
            invalid,
        )
        .expect_err("unsupported mutation rolls back receipt");
        assert!(matches!(
            failure,
            TransactionFailure::Operation(AuthenticatedInboundEntityError::Mutation(_))
        ));
        assert_eq!(count(&connection, "ark_sync_inbox"), 1);
    }

    #[test]
    fn authenticated_entity_keeps_unknown_object_pending() {
        let mut connection = setup();
        crate::canonical_migrations::objects::ensure_object_state_schema(&connection)
            .expect("canonical object state schema initializes");
        let entity = entity(
            "object",
            "object-1",
            serde_json::json!({
                "typeId":"custom.book", "typeVersion":"1.0.0", "title":"Book",
                "contentJson":{"type":"doc","content":[]}, "propsJson":{},
                "createdAt":"2026-10-20T00:00:00Z", "updatedAt":"2026-10-20T00:00:00Z",
                "deletedAt":null
            }),
            "2026-10-20T00:00:00.000Z:000001:peer",
            1,
        );
        let result: AuthenticatedInboundEntityOutcome =
            SqliteWriter::apply_authenticated_inbound_entity_on_connection(
                &mut connection,
                write("pending", &entity),
                entity,
            )
            .expect("unknown object type is pending");
        assert_eq!(result.outcome, InboundApplyOutcome::Pending);
        assert_eq!(result.state, InboxState::Pending);
        assert_eq!(result.pending_type_id.as_deref(), Some("custom.book"));
        assert_eq!(count(&connection, "ark_committed_feed"), 0);
    }
}
