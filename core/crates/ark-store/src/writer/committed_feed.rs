use super::{SqliteWriter, StoreTransactionFailure};
use ark_application::sync_storage::{
    OutboundProgressMutation, OutboundProgressPolicy, OutboundProgressWrite,
    OutboundProgressWriteError,
};

impl SqliteWriter {
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
    use crate::writer::SqliteWriter;
    use ark_application::sync_storage::{
        OutboundAck, OutboundProgressMutation as Mutation, OutboundProgressPolicy as Policy,
        OutboundProgressWrite,
    };
    use rusqlite::Connection;

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
