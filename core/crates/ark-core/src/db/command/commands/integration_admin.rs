use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::integration_replication::{
    persist_integration_grant_in_transaction, persist_node_authorization_in_transaction,
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
    NodeAuthorizationOperation,
};
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PersistNodeAuthorizationCommand {
    operation: NodeAuthorizationOperation,
    node: AuthorizedNode,
    grant: Option<IntegrationNodeGrant>,
    device_id: String,
}

impl PersistNodeAuthorizationCommand {
    pub(crate) fn new(
        operation: NodeAuthorizationOperation,
        node: AuthorizedNode,
        grant: Option<IntegrationNodeGrant>,
        device_id: String,
    ) -> Self {
        Self {
            operation,
            node,
            grant,
            device_id,
        }
    }
}

impl sealed::Sealed for PersistNodeAuthorizationCommand {}

impl DbCommand for PersistNodeAuthorizationCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        persist_node_authorization_in_transaction(
            transaction,
            self.operation,
            &self.node,
            self.grant.as_ref(),
            &self.device_id,
        )
        .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PersistIntegrationGrantCommand {
    grant: IntegrationNodeGrant,
    device_id: String,
}

impl PersistIntegrationGrantCommand {
    pub(crate) fn new(grant: IntegrationNodeGrant, device_id: String) -> Self {
        Self { grant, device_id }
    }
}

impl sealed::Sealed for PersistIntegrationGrantCommand {}

impl DbCommand for PersistIntegrationGrantCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        persist_integration_grant_in_transaction(transaction, &self.grant, &self.device_id)
            .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AcquireIntegrationRefreshLeaseCommand {
    params: crate::db::RefreshLeaseAcquireParams,
}

impl AcquireIntegrationRefreshLeaseCommand {
    pub(crate) fn new(params: crate::db::RefreshLeaseAcquireParams) -> Self {
        Self { params }
    }
}

impl sealed::Sealed for AcquireIntegrationRefreshLeaseCommand {}

impl DbCommand for AcquireIntegrationRefreshLeaseCommand {
    type Output = IntegrationRefreshLease;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::db::try_acquire_integration_refresh_lease_in_transaction(transaction, &self.params)
            .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PublishIntegrationCredentialEnvelopeCommand {
    envelope: IntegrationCredentialEnvelope,
    device_id: String,
    now_ms: u64,
}

impl PublishIntegrationCredentialEnvelopeCommand {
    pub(crate) fn new(
        envelope: IntegrationCredentialEnvelope,
        device_id: String,
        now_ms: u64,
    ) -> Self {
        Self {
            envelope,
            device_id,
            now_ms,
        }
    }
}

impl sealed::Sealed for PublishIntegrationCredentialEnvelopeCommand {}

impl DbCommand for PublishIntegrationCredentialEnvelopeCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        crate::db::publish_integration_credential_envelope_in_transaction(
            transaction,
            &self.envelope,
            &self.device_id,
            self.now_ms,
        )
        .map_err(DbError::from_application_error)
    }
}
