use super::{
    Connection, SqliteWriter, StoreTransactionFailure, TransactionFailure, WriteCapabilities,
};
use rusqlite::Transaction;

pub struct SignedIntegrationFrameWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn signed_integration_frames(&mut self) -> SignedIntegrationFrameWrites<'_, '_> {
        SignedIntegrationFrameWrites {
            transaction: self.transaction,
        }
    }
}

#[derive(Debug)]
pub enum SignedIntegrationFrameWriteError {
    PeerNotAdmitted,
    Apply(ark_domain::integration_replication::SignedSyncError),
}

impl SignedIntegrationFrameWrites<'_, '_> {
    pub fn apply(
        &mut self,
        authenticated_peer_id: &str,
        request: ark_application::signed_integration::ApplySignedIntegrationFrame,
    ) -> Result<(), SignedIntegrationFrameWriteError> {
        apply_signed_integration_frame_in(self.transaction, authenticated_peer_id, request)
    }
}

fn apply_signed_integration_frame_in(
    transaction: &Transaction<'_>,
    authenticated_peer_id: &str,
    request: ark_application::signed_integration::ApplySignedIntegrationFrame,
) -> Result<(), SignedIntegrationFrameWriteError> {
    let admitted = crate::device_registry::is_device_admitted(transaction, authenticated_peer_id)
        .map_err(|error| {
            SignedIntegrationFrameWriteError::Apply(
                ark_domain::integration_replication::SignedSyncError::Storage(error),
            )
        })?
        .unwrap_or(false);
    if !admitted {
        return Err(SignedIntegrationFrameWriteError::PeerNotAdmitted);
    }
    let mut session =
        crate::integration_replication::SqliteSignedIntegrationFrameSession::new(transaction);
    ark_application::signed_integration::apply_signed_integration_frame(&mut session, request)
        .map_err(SignedIntegrationFrameWriteError::Apply)
}

impl SqliteWriter {
    pub fn apply_signed_integration_frame(
        &mut self,
        authenticated_peer_id: &str,
        request: ark_application::signed_integration::ApplySignedIntegrationFrame,
    ) -> Result<(), StoreTransactionFailure<SignedIntegrationFrameWriteError>> {
        self.write_with_capabilities(|capabilities| {
            capabilities
                .signed_integration_frames()
                .apply(authenticated_peer_id, request)
        })
    }

    pub fn apply_signed_integration_frame_on_connection(
        connection: &mut Connection,
        authenticated_peer_id: &str,
        request: ark_application::signed_integration::ApplySignedIntegrationFrame,
    ) -> Result<(), TransactionFailure<SignedIntegrationFrameWriteError>> {
        Self::immediate_on_connection(connection, |transaction| {
            apply_signed_integration_frame_in(transaction, authenticated_peer_id, request)
        })
    }
}
