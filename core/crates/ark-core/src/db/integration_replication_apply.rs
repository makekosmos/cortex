#[cfg(test)]
use super::{get_sync_kv, VERSION_VECTOR_KEY};

#[cfg(test)]
type SignedIntegrationApplyResult = Result<(), crate::integration_replication::SignedSyncError>;

#[cfg(test)]
pub(in crate::db) fn apply_signed_integration_changes(
    conn: &rusqlite::Connection,
    envelope: &crate::integration_replication::SignedSyncEnvelope,
    expected_space_id: &str,
    expected_recipient_node_id: &str,
    authenticated_transport_public_key: Option<&str>,
    reserved_at_ms: u64,
) -> SignedIntegrationApplyResult {
    use crate::application::signed_integration::{
        apply_signed_integration_frame, ApplySignedIntegrationFrame,
    };
    use crate::db::SqliteSignedIntegrationFrameSession;
    use rusqlite::{Transaction, TransactionBehavior};

    if !conn.is_autocommit() {
        return Err(crate::integration_replication::SignedSyncError::Storage(
            "signed integration apply requires an autocommit connection".into(),
        ));
    }
    let transaction =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|error| {
            crate::integration_replication::SignedSyncError::Storage(error.to_string())
        })?;
    let mut session = SqliteSignedIntegrationFrameSession::new(&transaction);
    apply_signed_integration_frame(
        &mut session,
        ApplySignedIntegrationFrame {
            envelope: envelope.clone(),
            expected_space_id: expected_space_id.into(),
            expected_recipient_node_id: expected_recipient_node_id.into(),
            authenticated_transport_public_key: authenticated_transport_public_key
                .map(str::to_owned),
            reserved_at_ms,
        },
    )?;
    transaction.commit().map_err(|error| {
        crate::integration_replication::SignedSyncError::Storage(error.to_string())
    })
}

#[cfg(test)]
pub(in crate::db) fn load_version_vector(
    conn: &rusqlite::Connection,
) -> Result<crate::types::VersionVector, String> {
    get_sync_kv(conn, VERSION_VECTOR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(&value).map_err(|error| error.to_string()))
        .transpose()
        .map(Option::unwrap_or_default)
}
