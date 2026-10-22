use super::{SqliteWriter, StoreTransactionFailure};
use crate::committed_feed::InboxState;
use ark_application::sync_storage::{
    InboxReceiptWrite, InboxReceiptWriteError, OutboundProgressMutation, OutboundProgressPolicy,
    OutboundProgressWrite, OutboundProgressWriteError,
};
use ark_domain::types::MAX_SYNC_BATCH_BYTES;

impl SqliteWriter {
    pub fn receive_inbox_receipt(
        &mut self,
        write: InboxReceiptWrite,
    ) -> Result<crate::committed_feed::InboxState, StoreTransactionFailure<InboxReceiptWriteError>>
    {
        self.write_with_capabilities(|capabilities| {
            receive_inbox_receipt(capabilities.transaction, write)
        })
    }

    #[cfg(debug_assertions)]
    pub fn receive_inbox_receipt_on_connection(
        connection: &mut super::Connection,
        write: InboxReceiptWrite,
    ) -> Result<InboxState, super::TransactionFailure<InboxReceiptWriteError>> {
        Self::immediate_on_connection(connection, |transaction| {
            receive_inbox_receipt(transaction, write)
        })
    }

    pub fn apply_outbound_progress_write(
        &mut self,
        write: OutboundProgressWrite,
    ) -> Result<
        ark_application::sync_storage::OutboundProgress,
        StoreTransactionFailure<OutboundProgressWriteError>,
    > {
        self.write_with_capabilities(|capabilities| {
            apply_outbound_progress_write(capabilities.transaction, write)
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_outbound_progress_write_on_connection(
        connection: &mut super::Connection,
        write: OutboundProgressWrite,
    ) -> Result<
        ark_application::sync_storage::OutboundProgress,
        super::TransactionFailure<OutboundProgressWriteError>,
    > {
        Self::immediate_on_connection(connection, |transaction| {
            apply_outbound_progress_write(transaction, write)
        })
    }
}

fn receive_inbox_receipt(
    transaction: &super::Transaction<'_>,
    write: InboxReceiptWrite,
) -> Result<InboxState, InboxReceiptWriteError> {
    if write.payload_json.len() > MAX_SYNC_BATCH_BYTES {
        return Err(InboxReceiptWriteError::Inbox(
            "sync_inbox_payload_limit_exceeded".into(),
        ));
    }

    if let Some(policy) = &write.policy {
        let profile =
            crate::data_platform::load_sync_profile_for_sync(transaction, &policy.local_device_id)
                .map_err(InboxReceiptWriteError::PolicyStorage)?;
        if ark_application::sync_policy::sync_profile_identity(profile.as_ref())
            != policy.expected_identity
        {
            return Err(InboxReceiptWriteError::PolicyChanged);
        }
    }

    let admitted = crate::device_registry::is_device_admitted(transaction, &write.peer_device_id)
        .map_err(InboxReceiptWriteError::RegistryStorage)?
        .unwrap_or(false);
    if !admitted {
        return Err(InboxReceiptWriteError::PeerNotAdmitted);
    }

    let removed = crate::sync_kv::get_sync_kv(transaction, "sync.removed_peers")
        .map_err(InboxReceiptWriteError::RemovedPeersStorage)?
        .map(|raw| serde_json::from_str::<Vec<String>>(&raw))
        .transpose()
        .map_err(|error| InboxReceiptWriteError::RemovedPeersStorage(error.to_string()))?
        .unwrap_or_default();
    if removed.iter().any(|peer| peer == &write.peer_device_id) {
        return Err(InboxReceiptWriteError::PeerRemoved);
    }

    crate::committed_feed::receive_inbox_payload(
        transaction,
        &write.peer_device_id,
        &write.message_id,
        &write.payload_json,
    )
    .map_err(InboxReceiptWriteError::Inbox)
}

fn apply_outbound_progress_write(
    transaction: &super::Transaction<'_>,
    write: OutboundProgressWrite,
) -> Result<ark_application::sync_storage::OutboundProgress, OutboundProgressWriteError> {
    let generation = match &write.policy {
        OutboundProgressPolicy::Generation(generation) => *generation,
        OutboundProgressPolicy::Identity(expected) => {
            let current = crate::data_platform::load_accepted_peer_sync_policy(
                transaction,
                &write.peer_device_id,
            )
            .map_err(OutboundProgressWriteError::PolicyStorage)?
            .map_or_else(
                ark_application::sync_policy::default_sync_policy_identity,
                |policy| policy.identity(),
            );
            if current != *expected {
                return Err(OutboundProgressWriteError::PolicyChanged);
            }
            expected.revision
        }
    };

    use OutboundProgressMutation as Mutation;
    let progress = match write.mutation {
        Mutation::Begin => crate::committed_feed::begin_outbound_policy(
            transaction,
            &write.peer_device_id,
            generation,
        ),
        Mutation::CompleteSnapshot => crate::committed_feed::complete_outbound_snapshot(
            transaction,
            &write.peer_device_id,
            generation,
        ),
        Mutation::RecordScan { scanned_through } => crate::committed_feed::record_outbound_scan(
            transaction,
            &write.peer_device_id,
            generation,
            scanned_through,
        ),
        Mutation::Acknowledge { position, ack } => crate::committed_feed::acknowledge_outbound(
            transaction,
            &write.peer_device_id,
            generation,
            position,
            ack,
        ),
    };
    progress.map_err(OutboundProgressWriteError::ProgressStorage)
}

#[cfg(test)]
mod tests {
    use crate::writer::{SqliteWriter, TransactionFailure};
    use ark_application::sync_storage::{
        OutboundAck, OutboundProgressMutation as Mutation, OutboundProgressPolicy as Policy,
        OutboundProgressWrite,
    };
    use rusqlite::Connection;

    #[test]
    fn inbox_receipt_writer_checks_admission_and_persists_idempotently() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        let mut connection = connection;
        let write = || ark_application::sync_storage::InboxReceiptWrite {
            peer_device_id: "peer-a".into(),
            message_id: "message-1".into(),
            payload_json: "{\"x\":1}".into(),
            policy: None,
        };

        let denied = SqliteWriter::receive_inbox_receipt_on_connection(&mut connection, write())
            .expect_err("unknown peers cannot persist receipts");
        assert!(matches!(
            denied,
            TransactionFailure::Operation(
                ark_application::sync_storage::InboxReceiptWriteError::PeerNotAdmitted
            )
        ));
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ark_sync_inbox", [], |row| row.get(0))
            .expect("receipt count reads");
        assert_eq!(count, 0);

        connection
            .execute(
                "INSERT INTO device_registry_projection(
                    device_id, signing_public_key, key_epoch, admission_epoch
                 ) VALUES ('peer-a', zeroblob(32), 1, 1)",
                [],
            )
            .expect("peer is admitted");
        assert_eq!(
            SqliteWriter::receive_inbox_receipt_on_connection(&mut connection, write())
                .expect("receipt persists"),
            crate::committed_feed::InboxState::Received
        );
        assert_eq!(
            SqliteWriter::receive_inbox_receipt_on_connection(&mut connection, write())
                .expect("duplicate receipt preserves state"),
            crate::committed_feed::InboxState::Received
        );

        crate::sync_kv::set_sync_kv(&connection, "sync.removed_peers", "[\"peer-a\"]")
            .expect("removed peer list persists");
        let removed = SqliteWriter::receive_inbox_receipt_on_connection(
            &mut connection,
            ark_application::sync_storage::InboxReceiptWrite {
                message_id: "message-2".into(),
                ..write()
            },
        )
        .expect_err("removed peers cannot persist new receipts");
        assert!(matches!(
            removed,
            TransactionFailure::Operation(
                ark_application::sync_storage::InboxReceiptWriteError::PeerRemoved
            )
        ));

        let oversized = SqliteWriter::receive_inbox_receipt_on_connection(
            &mut connection,
            ark_application::sync_storage::InboxReceiptWrite {
                message_id: "oversized".into(),
                payload_json: "x".repeat(ark_domain::types::MAX_SYNC_BATCH_BYTES + 1),
                ..write()
            },
        )
        .expect_err("oversized payloads are rejected");
        assert!(matches!(
            oversized,
            TransactionFailure::Operation(
                ark_application::sync_storage::InboxReceiptWriteError::Inbox(ref message)
            ) if message == "sync_inbox_payload_limit_exceeded"
        ));
    }

    #[test]
    fn generation_progress_writes_preserve_snapshot_scan_and_ack_transitions() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("base schema creates");
        crate::data_platform::ensure_schema(&connection).expect("sync policy schema creates");
        let mut writer = SqliteWriter::from_connection(connection);

        let apply = |writer: &mut SqliteWriter, mutation| {
            writer.apply_outbound_progress_write(OutboundProgressWrite::new(
                "peer-a".into(),
                Policy::Generation(4),
                mutation,
            ))
        };

        let begun = apply(&mut writer, Mutation::Begin).expect("progress begins");
        assert!(!begun.initial_snapshot.is_complete());
        let complete =
            apply(&mut writer, Mutation::CompleteSnapshot).expect("initial snapshot completes");
        assert!(complete.initial_snapshot.is_complete());

        for index in 1..=3 {
            writer
                .with_connection_mut(|connection| {
                    crate::committed_feed::append_committed_change(
                        connection,
                        "object",
                        &format!("object-{index}"),
                        "revision-1",
                        false,
                        None,
                        None,
                    )
                })
                .expect("committed change appends");
        }

        let scanned = apply(&mut writer, Mutation::RecordScan { scanned_through: 3 })
            .expect("scan cursor advances");
        assert_eq!(scanned.scanned_through, 3);
        let acknowledged = apply(
            &mut writer,
            Mutation::Acknowledge {
                position: 2,
                ack: OutboundAck::DurableReceipt,
            },
        )
        .expect("receipt cursor advances");
        assert_eq!(acknowledged.receipt_through, 2);
        assert_eq!(acknowledged.applied_through, 0);
    }
}
