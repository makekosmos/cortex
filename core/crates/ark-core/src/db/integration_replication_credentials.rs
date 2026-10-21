use crate::integration_replication::read_store::IntegrationReplicationReadPort;
use crate::integration_replication::{
    validate_envelope_recipient, IntegrationContractError, IntegrationCredentialEnvelope,
};

pub fn load_integration_credential_envelope<S: IntegrationReplicationReadPort>(
    conn: &S,
    integration_id: &str,
    recipient_node_id: &str,
    credential_generation: u64,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let Some(envelope) =
        conn.credential_envelope(integration_id, recipient_node_id, credential_generation)?
    else {
        return Ok(None);
    };
    validate_loaded_envelope_recipient(conn, &envelope)?;
    Ok(Some(envelope))
}

pub fn load_latest_integration_credential_envelope<S: IntegrationReplicationReadPort>(
    conn: &S,
    integration_id: &str,
    recipient_node_id: &str,
) -> Result<Option<IntegrationCredentialEnvelope>, String> {
    let Some(envelope) = conn.latest_credential_envelope(integration_id, recipient_node_id)? else {
        return Ok(None);
    };
    validate_loaded_envelope_recipient(conn, &envelope)?;
    Ok(Some(envelope))
}

fn validate_loaded_envelope_recipient<S: IntegrationReplicationReadPort>(
    conn: &S,
    envelope: &IntegrationCredentialEnvelope,
) -> Result<(), String> {
    let node = conn
        .authorized_node(&envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
    let grant = conn
        .grant(&envelope.integration_id, &envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    validate_envelope_recipient(envelope, &node, &grant).map_err(|error| error.to_string())
}
