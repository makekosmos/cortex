use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::integration_replication::{
    persist_integration_grant_in_transaction, persist_node_authorization_in_transaction,
    AuthorizedNode, IntegrationNodeGrant, NodeAuthorizationOperation,
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
