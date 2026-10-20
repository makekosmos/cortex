use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use crate::integration_replication::{SignedSyncEnvelope, SignedSyncError};
use rusqlite::{Connection, Transaction};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) enum ValidateOutboundSignedFrameQuery {
    Authorized {
        frame: SignedSyncEnvelope,
        expected_space_id: String,
        expected_origin_node_id: String,
    },
    BoundTransport {
        frame: SignedSyncEnvelope,
        expected_space_id: String,
        expected_origin_node_id: String,
        transport_public_key: String,
    },
}

impl sealed::Sealed for ValidateOutboundSignedFrameQuery {}

impl DbQuery for ValidateOutboundSignedFrameQuery {
    type Output = ();

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        let result = match self {
            Self::Authorized {
                frame,
                expected_space_id,
                expected_origin_node_id,
            } => crate::integration_replication::validate_outbound_signed_sync(
                connection,
                &expected_space_id,
                &expected_origin_node_id,
                &frame,
            ),
            Self::BoundTransport {
                frame,
                expected_space_id,
                expected_origin_node_id,
                transport_public_key,
            } => crate::integration_replication::validate_outbound_signed_sync_with_transport(
                connection,
                &expected_space_id,
                &expected_origin_node_id,
                &frame,
                &transport_public_key,
            ),
        };
        result.map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) enum IdentityErrorStyle {
    Detailed,
    Generic,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplySignedIntegrationFrameCommand {
    frame: SignedSyncEnvelope,
    expected_space_id: String,
    expected_recipient_node_id: String,
    authenticated_transport_public_key: Option<String>,
    identity_error_style: IdentityErrorStyle,
}

impl ApplySignedIntegrationFrameCommand {
    pub(crate) fn new(
        frame: SignedSyncEnvelope,
        expected_space_id: String,
        expected_recipient_node_id: String,
        authenticated_transport_public_key: Option<String>,
        identity_error_style: IdentityErrorStyle,
    ) -> Self {
        Self {
            frame,
            expected_space_id,
            expected_recipient_node_id,
            authenticated_transport_public_key,
            identity_error_style,
        }
    }
}

impl sealed::Sealed for ApplySignedIntegrationFrameCommand {}

impl DbCommand for ApplySignedIntegrationFrameCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let authenticated_peer_id = context.authenticated_peer_id()?;
        super::device_registry_sync::require_registry_peer_admitted(
            transaction,
            authenticated_peer_id,
        )?;
        let mismatch = |message: &str| DbError::from_application_error(message.into());
        let identity_matches = self.frame.space_id == self.expected_space_id
            && self.frame.origin_node_id == authenticated_peer_id
            && self.frame.recipient_node_id == self.expected_recipient_node_id;
        if !identity_matches {
            return Err(match self.identity_error_style {
                IdentityErrorStyle::Detailed if self.frame.space_id != self.expected_space_id => {
                    mismatch("signed integration frame has the wrong space")
                }
                IdentityErrorStyle::Detailed
                    if self.frame.origin_node_id != authenticated_peer_id =>
                {
                    mismatch("signed integration frame origin is not the authenticated peer")
                }
                IdentityErrorStyle::Detailed => {
                    mismatch("signed integration frame is addressed to another node")
                }
                IdentityErrorStyle::Generic => {
                    mismatch("signed integration frame identity mismatch")
                }
            });
        }

        let reserved_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);
        let mut session = crate::db::SqliteSignedIntegrationFrameSession::new(transaction);
        ark_application::signed_integration::apply_signed_integration_frame(
            &mut session,
            ark_application::signed_integration::ApplySignedIntegrationFrame {
                envelope: self.frame,
                expected_space_id: self.expected_space_id,
                expected_recipient_node_id: self.expected_recipient_node_id,
                authenticated_transport_public_key: self.authenticated_transport_public_key,
                reserved_at_ms,
            },
        )
        .map_err(|error: SignedSyncError| DbError::from_application_error(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_transport_frame_fits_query_and_command_admission_weights() {
        const FRAME_LIMIT: usize = 16 * 1024 * 1024;
        let mut frame = SignedSyncEnvelope::new("s", "o", "r", 1, "m", Vec::new(), "");
        let fixed_json_len = serde_json::to_vec(&frame).unwrap().len();
        frame.signature = "x".repeat(FRAME_LIMIT - fixed_json_len);
        let wire_len = serde_json::to_vec(&frame).unwrap().len();
        assert_eq!(wire_len, FRAME_LIMIT);

        let query = ValidateOutboundSignedFrameQuery::Authorized {
            frame: frame.clone(),
            expected_space_id: "s".into(),
            expected_origin_node_id: "o".into(),
        };
        let command = ApplySignedIntegrationFrameCommand::new(
            frame,
            "s".into(),
            "r".into(),
            None,
            IdentityErrorStyle::Detailed,
        );
        assert!(query.weight() <= super::super::super::MAX_COMMAND_BYTES);
        assert!(command.weight() <= super::super::super::MAX_COMMAND_BYTES);
    }
}
