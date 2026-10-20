use ark_application::integration_admin::IntegrationAdminStore;
use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationCredentialEnvelope, IntegrationNodeGrant, IntegrationRefreshLease,
};
use rusqlite::Connection;

pub(crate) struct SqliteIntegrationAdminStore<'a> {
    connection: &'a Connection,
}

impl<'a> SqliteIntegrationAdminStore<'a> {
    pub(crate) fn new(connection: &'a Connection) -> Self {
        Self { connection }
    }
}

impl IntegrationAdminStore for SqliteIntegrationAdminStore<'_> {
    fn load_node(&self, id: &str) -> Result<Option<AuthorizedNode>, String> {
        crate::integration_replication::load_authorized_node(self.connection, id)
    }

    fn load_grant(
        &self,
        integration: &str,
        node: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String> {
        crate::integration_replication::load_integration_node_grant(
            self.connection,
            integration,
            node,
        )
    }

    fn load_lease(&self, integration: &str) -> Result<Option<IntegrationRefreshLease>, String> {
        crate::integration_replication::load_integration_refresh_lease(self.connection, integration)
    }

    fn max_generation(&self, integration: &str) -> Result<Option<u64>, String> {
        crate::integration_replication::load_max_integration_credential_generation(
            self.connection,
            integration,
        )
    }

    fn load_envelope(
        &self,
        integration: &str,
        recipient: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String> {
        crate::integration_replication::load_integration_credential_envelope_record(
            self.connection,
            integration,
            recipient,
            generation,
        )
    }

    fn put_node(&self, node: &AuthorizedNode, device: &str) -> Result<(), String> {
        persist_with_vector(
            self.connection,
            "authorized_node",
            &node.node_id,
            device,
            || crate::integration_replication::upsert_authorized_node_record(self.connection, node),
        )
    }

    fn put_grant(&self, grant: &IntegrationNodeGrant, device: &str) -> Result<(), String> {
        let entity_id = ark_domain::integration_replication::integration_node_id(
            &grant.integration_id,
            &grant.node_id,
        );
        persist_with_vector(
            self.connection,
            "integration_node_grant",
            &entity_id,
            device,
            || {
                crate::integration_replication::upsert_integration_node_grant_record(
                    self.connection,
                    grant,
                )
            },
        )
    }

    fn put_lease(&self, lease: &IntegrationRefreshLease, device: &str) -> Result<(), String> {
        persist_with_vector(
            self.connection,
            "integration_refresh_lease",
            &lease.integration_id,
            device,
            || {
                crate::integration_replication::upsert_integration_refresh_lease_record(
                    self.connection,
                    lease,
                )
            },
        )
    }

    fn put_envelope(
        &self,
        envelope: &IntegrationCredentialEnvelope,
        device: &str,
        _now_ms: u64,
    ) -> Result<(), String> {
        let entity_id = ark_domain::integration_replication::envelope_entity_id(
            &envelope.integration_id,
            &envelope.recipient_node_id,
            envelope.credential_generation,
        );
        persist_with_vector(
            self.connection,
            "integration_credential_envelope",
            &entity_id,
            device,
            || {
                crate::integration_replication::upsert_integration_credential_envelope_record(
                    self.connection,
                    envelope,
                )
            },
        )
    }
}

pub(super) fn persist_with_vector(
    connection: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    write: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("device_id must not be empty".into());
    }
    const SAVEPOINT: &str = "ark_integration_admin_write";
    connection
        .execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(|error| error.to_string())?;
    let result = (|| {
        write()?;
        let vector_id = format!("{entity_type}:{entity_id}");
        crate::sync_versions::bump_sync_version_vector(
            connection,
            entity_type,
            &vector_id,
            device_id,
            false,
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => match connection.execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}")) {
            Ok(()) => Ok(()),
            Err(error) => {
                let rollback = connection.execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
                ));
                match rollback {
                    Ok(()) => Err(error.to_string()),
                    Err(rollback_error) => Err(format!(
                        "{}; savepoint rollback failed: {}",
                        error, rollback_error
                    )),
                }
            }
        },
        Err(error) => {
            connection
                .execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
                ))
                .map_err(|rollback_error| {
                    format!("{error}; savepoint rollback failed: {rollback_error}")
                })?;
            Err(error)
        }
    }
}
