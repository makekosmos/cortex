// Test-only compatibility helpers exercise the same Store writer API as the
// production actor without restoring SQLite access to ark-core production code.
#[cfg(test)]
fn begin_outbound_policy_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_test(
        connection,
        peer_device_id,
        identity,
        ark_application::sync_storage::OutboundProgressMutation::Begin,
    )
}

#[cfg(test)]
fn complete_outbound_snapshot_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_test(
        connection,
        peer_device_id,
        identity,
        ark_application::sync_storage::OutboundProgressMutation::CompleteSnapshot,
    )
}

#[cfg(test)]
fn record_outbound_scan_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    scanned_through: i64,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_test(
        connection,
        peer_device_id,
        identity,
        ark_application::sync_storage::OutboundProgressMutation::RecordScan { scanned_through },
    )
}

#[cfg(test)]
fn acknowledge_outbound_identity(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    position: i64,
    ack: OutboundAck,
) -> Result<OutboundProgress, String> {
    apply_outbound_progress_test(
        connection,
        peer_device_id,
        identity,
        ark_application::sync_storage::OutboundProgressMutation::Acknowledge { position, ack },
    )
}

#[cfg(test)]
fn apply_outbound_progress_test(
    connection: &mut Connection,
    peer_device_id: &str,
    identity: &ark_application::sync_storage::PolicyIdentity,
    mutation: ark_application::sync_storage::OutboundProgressMutation,
) -> Result<OutboundProgress, String> {
    let write = ark_application::sync_storage::OutboundProgressWrite::new(
        peer_device_id.to_owned(),
        ark_application::sync_storage::OutboundProgressPolicy::Identity(identity.clone()),
        mutation,
    );
    ark_store::writer::SqliteWriter::apply_outbound_progress_write_on_connection(connection, write)
        .map_err(|failure| match failure {
            ark_store::writer::TransactionFailure::Operation(error) => error.to_string(),
            other => format!("{other:?}"),
        })
}
