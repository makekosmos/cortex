use super::*;
use crate::writer::SqliteWriter;
use ark_application::sync_storage::{
    OutboundAck, OutboundProgress, OutboundProgressMutation, OutboundProgressPolicy,
    OutboundProgressWrite, PolicyIdentity,
};

pub(super) const MAX_FEED_DELIVERY_BYTES: usize =
    ark_application::sync_storage::MAX_SYNC_BATCH_BYTES - 512;

pub(super) fn apply_outbound_progress_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    mutation: OutboundProgressMutation,
) -> Result<OutboundProgress, String> {
    let write = OutboundProgressWrite::new(
        peer_device_id.to_owned(),
        OutboundProgressPolicy::Identity(identity.clone()),
        mutation,
    );
    SqliteWriter::apply_outbound_progress_write_on_connection(connection, write).map_err(
        |failure| match failure {
            crate::writer::TransactionFailure::Operation(error) => error.to_string(),
            other => format!("{other:?}"),
        },
    )
}

pub(super) fn begin_outbound_policy_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &PolicyIdentity,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_identity(
        connection,
        peer_device_id,
        identity,
        OutboundProgressMutation::Begin,
    )
}

pub(super) fn complete_outbound_snapshot_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &PolicyIdentity,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_identity(
        connection,
        peer_device_id,
        identity,
        OutboundProgressMutation::CompleteSnapshot,
    )
}

pub(super) fn record_outbound_scan_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    scanned_through: i64,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_identity(
        connection,
        peer_device_id,
        identity,
        OutboundProgressMutation::RecordScan { scanned_through },
    )
}

pub(super) fn acknowledge_outbound_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    position: i64,
    ack: OutboundAck,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_identity(
        connection,
        peer_device_id,
        identity,
        OutboundProgressMutation::Acknowledge { position, ack },
    )
}
