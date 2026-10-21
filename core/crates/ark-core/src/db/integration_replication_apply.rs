#[cfg(test)]
type SignedIntegrationApplyResult = Result<(), crate::integration_replication::SignedSyncError>;

#[cfg(test)]
pub(in crate::db) fn apply_signed_integration_changes(
    conn: &mut rusqlite::Connection,
    envelope: &crate::integration_replication::SignedSyncEnvelope,
    expected_space_id: &str,
    expected_recipient_node_id: &str,
    authenticated_transport_public_key: Option<&str>,
    reserved_at_ms: u64,
) -> SignedIntegrationApplyResult {
    use crate::integration_replication::SignedSyncError;
    use ark_application::signed_integration::ApplySignedIntegrationFrame;
    use ark_store::writer::{SignedIntegrationFrameWriteError, SqliteWriter, TransactionFailure};
    use rusqlite::params;

    let authorized =
        ark_store::integration_replication::load_authorized_node(conn, &envelope.origin_node_id)
            .map_err(SignedSyncError::Storage)?
            .ok_or_else(|| SignedSyncError::Apply("authenticated peer is not authorized".into()))?;
    let signing_key = decode_public_key(&authorized.signing_public_key)?;
    conn.execute(
        "INSERT OR IGNORE INTO device_registry_projection
         (device_id, signing_public_key, key_epoch, admission_epoch, display_name,
          disabled_event_id, display_name_hlc, updated_hlc, updated_event_id)
         VALUES (?1, ?2, 1, 1, NULL, NULL, NULL, NULL, NULL)",
        params![&envelope.origin_node_id, signing_key],
    )
    .map_err(|error| SignedSyncError::Storage(error.to_string()))?;

    let request = ApplySignedIntegrationFrame {
        envelope: envelope.clone(),
        expected_space_id: expected_space_id.into(),
        expected_recipient_node_id: expected_recipient_node_id.into(),
        authenticated_transport_public_key: authenticated_transport_public_key.map(str::to_owned),
        reserved_at_ms,
    };
    SqliteWriter::apply_signed_integration_frame_on_connection(
        conn,
        &envelope.origin_node_id,
        request,
    )
    .map_err(|failure| match failure {
        TransactionFailure::Begin(error) | TransactionFailure::Commit(error) => {
            SignedSyncError::Storage(error.to_string())
        }
        TransactionFailure::Operation(SignedIntegrationFrameWriteError::Apply(error)) => error,
        TransactionFailure::Operation(SignedIntegrationFrameWriteError::PeerNotAdmitted) => {
            SignedSyncError::Apply("authenticated peer is not admitted".into())
        }
    })
}

fn decode_public_key(
    value: &str,
) -> Result<Vec<u8>, crate::integration_replication::SignedSyncError> {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return Err(crate::integration_replication::SignedSyncError::Storage(
            "authorized node signing key must be 32-byte hex".into(),
        ));
    }
    (0..bytes.len())
        .step_by(2)
        .map(|index| {
            let pair = std::str::from_utf8(&bytes[index..index + 2]).map_err(|error| {
                crate::integration_replication::SignedSyncError::Storage(error.to_string())
            })?;
            u8::from_str_radix(pair, 16).map_err(|error| {
                crate::integration_replication::SignedSyncError::Storage(error.to_string())
            })
        })
        .collect()
}
