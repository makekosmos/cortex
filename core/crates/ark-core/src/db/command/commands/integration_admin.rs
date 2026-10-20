use super::super::{CommandContext, DbError};
use crate::integration_replication::{
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, NodeAuthorizationOperation,
};
pub(crate) use ark_application::integration_admin::IntegrationAdminOutput;
use ark_application::integration_admin::{
    IntegrationAdminWrite, PersistIntegrationGrant, PersistNodeAuthorization,
    PublishCredentialEnvelope, RefreshLeaseAcquire,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) enum IntegrationAdminCommand {
    PersistNodeAuthorization {
        operation: NodeAuthorizationOperation,
        node: AuthorizedNode,
        grant: Option<IntegrationNodeGrant>,
        device_id: String,
    },
    PersistGrant {
        grant: IntegrationNodeGrant,
        device_id: String,
    },
    AcquireRefreshLease(crate::db::RefreshLeaseAcquireParams),
    PublishCredentialEnvelope {
        envelope: IntegrationCredentialEnvelope,
        device_id: String,
        now_ms: u64,
    },
}

impl IntegrationAdminCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    pub(crate) fn execute(
        self,
        context: &CommandContext,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<IntegrationAdminOutput, DbError> {
        context.require_engine()?;
        writer
            .apply_integration_admin(self.into_write())
            .map_err(map_store_failure)
    }

    #[cfg(debug_assertions)]
    pub(crate) fn execute_on_connection(
        self,
        context: &CommandContext,
        connection: &mut rusqlite::Connection,
    ) -> Result<IntegrationAdminOutput, DbError> {
        context.require_engine()?;
        ark_store::writer::apply_integration_admin_on_connection(connection, self.into_write())
            .map_err(map_transaction_failure)
    }

    fn into_write(self) -> IntegrationAdminWrite {
        match self {
            Self::PersistNodeAuthorization {
                operation,
                node,
                grant,
                device_id,
            } => IntegrationAdminWrite::PersistNodeAuthorization(PersistNodeAuthorization {
                operation: operation.into(),
                node,
                grant,
                device_id,
            }),
            Self::PersistGrant { grant, device_id } => {
                IntegrationAdminWrite::PersistIntegrationGrant(PersistIntegrationGrant {
                    grant,
                    device_id,
                })
            }
            Self::AcquireRefreshLease(params) => {
                IntegrationAdminWrite::AcquireRefreshLease(RefreshLeaseAcquire {
                    integration_id: params.integration_id,
                    holder_node_id: params.holder_node_id,
                    credential_generation: params.credential_generation,
                    now_ms: params.now_ms,
                    ttl_ms: params.ttl_ms,
                    expected_fencing_token: params.expected_fencing_token,
                    device_id: params.device_id,
                })
            }
            Self::PublishCredentialEnvelope {
                envelope,
                device_id,
                now_ms,
            } => IntegrationAdminWrite::PublishCredentialEnvelope(PublishCredentialEnvelope {
                envelope,
                device_id,
                now_ms,
            }),
        }
    }
}

impl From<NodeAuthorizationOperation>
    for ark_application::integration_admin::NodeAuthorizationOperation
{
    fn from(value: NodeAuthorizationOperation) -> Self {
        match value {
            NodeAuthorizationOperation::Authorize => Self::Authorize,
            NodeAuthorizationOperation::Revoke => Self::Revoke,
            NodeAuthorizationOperation::Rotate => Self::Rotate,
        }
    }
}

fn map_store_failure(error: ark_store::writer::StoreTransactionFailure<String>) -> DbError {
    match error {
        ark_store::writer::StoreTransactionFailure::Begin(error) => {
            DbError::Storage(error.to_string())
        }
        ark_store::writer::StoreTransactionFailure::Operation(error) => {
            DbError::from_application_error(error)
        }
        ark_store::writer::StoreTransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}

#[cfg(debug_assertions)]
fn map_transaction_failure(error: ark_store::writer::TransactionFailure<String>) -> DbError {
    match error {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_store_error(error),
        ark_store::writer::TransactionFailure::Operation(error) => {
            DbError::from_application_error(error)
        }
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
