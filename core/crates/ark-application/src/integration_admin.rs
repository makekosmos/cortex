use ark_domain::integration_replication::{
    AuthorizedNode, IntegrationContractError, IntegrationCredentialEnvelope, IntegrationNodeGrant,
    IntegrationRefreshLease,
};
mod validation;
use validation::{require_active_node_grant, validate_grant, validate_node_transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum NodeAuthorizationOperation {
    Authorize,
    Revoke,
    Rotate,
}

#[derive(Debug, Clone)]
pub struct PersistNodeAuthorization {
    pub operation: NodeAuthorizationOperation,
    pub node: AuthorizedNode,
    pub grant: Option<IntegrationNodeGrant>,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct PersistIntegrationGrant {
    pub grant: IntegrationNodeGrant,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct RefreshLeaseAcquire {
    pub integration_id: String,
    pub holder_node_id: String,
    pub credential_generation: u64,
    pub now_ms: u64,
    pub ttl_ms: u64,
    pub expected_fencing_token: u64,
    pub device_id: String,
}

#[derive(Debug, Clone)]
pub struct PublishCredentialEnvelope {
    pub envelope: IntegrationCredentialEnvelope,
    pub device_id: String,
    pub now_ms: u64,
}

#[derive(Debug, Clone)]
pub enum IntegrationAdminWrite {
    PersistNodeAuthorization(PersistNodeAuthorization),
    PersistIntegrationGrant(PersistIntegrationGrant),
    AcquireRefreshLease(RefreshLeaseAcquire),
    PublishCredentialEnvelope(PublishCredentialEnvelope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrationAdminOutput {
    Complete,
    Lease(IntegrationRefreshLease),
}

pub trait IntegrationAdminStore {
    fn load_node(&self, id: &str) -> Result<Option<AuthorizedNode>, String>;
    fn load_grant(
        &self,
        integration: &str,
        node: &str,
    ) -> Result<Option<IntegrationNodeGrant>, String>;
    fn load_lease(&self, integration: &str) -> Result<Option<IntegrationRefreshLease>, String>;
    fn max_generation(&self, integration: &str) -> Result<Option<u64>, String>;
    fn load_envelope(
        &self,
        integration: &str,
        recipient: &str,
        generation: u64,
    ) -> Result<Option<IntegrationCredentialEnvelope>, String>;
    fn put_node(&self, node: &AuthorizedNode, device: &str) -> Result<(), String>;
    fn put_grant(&self, grant: &IntegrationNodeGrant, device: &str) -> Result<(), String>;
    fn put_lease(&self, lease: &IntegrationRefreshLease, device: &str) -> Result<(), String>;
    fn put_envelope(
        &self,
        envelope: &IntegrationCredentialEnvelope,
        device: &str,
        now_ms: u64,
    ) -> Result<(), String>;
}

pub fn apply(
    store: &dyn IntegrationAdminStore,
    write: IntegrationAdminWrite,
) -> Result<IntegrationAdminOutput, String> {
    match write {
        IntegrationAdminWrite::PersistNodeAuthorization(request) => {
            persist_node_authorization(store, request)?;
            Ok(IntegrationAdminOutput::Complete)
        }
        IntegrationAdminWrite::PersistIntegrationGrant(request) => {
            persist_integration_grant(store, request)?;
            Ok(IntegrationAdminOutput::Complete)
        }
        IntegrationAdminWrite::AcquireRefreshLease(request) => {
            acquire_refresh_lease(store, request).map(IntegrationAdminOutput::Lease)
        }
        IntegrationAdminWrite::PublishCredentialEnvelope(request) => {
            publish_credential_envelope(store, request)?;
            Ok(IntegrationAdminOutput::Complete)
        }
    }
}

pub fn persist_node_authorization(
    store: &dyn IntegrationAdminStore,
    request: PersistNodeAuthorization,
) -> Result<(), String> {
    let current = store.load_node(&request.node.node_id)?;
    validate_node_transition(
        request.operation,
        current.as_ref(),
        &request.node,
        request.grant.as_ref(),
    )?;
    if current.as_ref() != Some(&request.node) {
        store.put_node(&request.node, &request.device_id)?;
    }
    if let Some(grant) = &request.grant {
        validate_grant(store, grant)?;
        store.put_grant(grant, &request.device_id)?;
    }
    Ok(())
}

pub fn persist_integration_grant(
    store: &dyn IntegrationAdminStore,
    request: PersistIntegrationGrant,
) -> Result<(), String> {
    let node = store
        .load_node(&request.grant.node_id)?
        .ok_or_else(|| "integration grant node is not authorized".to_owned())?;
    if request.grant.node_encryption_key != node.encryption_public_key
        || request.grant.grant_epoch < node.grant_epoch
    {
        return Err("integration grant does not match the node key epoch".into());
    }
    validate_grant(store, &request.grant)?;
    store.put_grant(&request.grant, &request.device_id)
}

pub fn acquire_refresh_lease(
    store: &dyn IntegrationAdminStore,
    request: RefreshLeaseAcquire,
) -> Result<IntegrationRefreshLease, String> {
    if request.holder_node_id != request.device_id {
        return Err("refresh lease holder must be the local node".into());
    }
    if store
        .max_generation(&request.integration_id)?
        .is_some_and(|v| request.credential_generation <= v)
    {
        return Err(IntegrationContractError::NonMonotonic {
            field: "credential_generation",
        }
        .to_string());
    }
    let current = store.load_lease(&request.integration_id)?;
    if current.as_ref().map_or(0, |lease| lease.fencing_token) != request.expected_fencing_token {
        return Err(IntegrationContractError::Mismatch {
            field: "expected_fencing_token",
        }
        .to_string());
    }
    let node = store
        .load_node(&request.holder_node_id)?
        .ok_or_else(|| "refresh lease holder is not authorized".to_owned())?;
    let grant = store
        .load_grant(&request.integration_id, &request.holder_node_id)?
        .ok_or_else(|| "integration grant does not exist".to_owned())?;
    require_active_node_grant(&node, &grant)?;
    let lease = ark_domain::integration_replication::acquire_refresh_lease(
        current.as_ref(),
        &request.integration_id,
        &request.holder_node_id,
        request.credential_generation,
        request.now_ms,
        request.ttl_ms,
    )
    .map_err(|e| e.to_string())?;
    store.put_lease(&lease, &request.device_id)?;
    Ok(lease)
}

pub fn publish_credential_envelope(
    store: &dyn IntegrationAdminStore,
    request: PublishCredentialEnvelope,
) -> Result<(), String> {
    let envelope = &request.envelope;
    if envelope.issuer_node_id != request.device_id {
        return Err("credential envelope issuer must be the local node".into());
    }
    envelope.validate().map_err(|e| e.to_string())?;
    let recipient = store
        .load_node(&envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "authorized_node",
            }
            .to_string()
        })?;
    let recipient_grant = store
        .load_grant(&envelope.integration_id, &envelope.recipient_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "integration_node_grant",
            }
            .to_string()
        })?;
    ark_domain::integration_replication::validate_envelope_recipient(
        envelope,
        &recipient,
        &recipient_grant,
    )
    .map_err(|e| e.to_string())?;
    let issuer = store.load_node(&envelope.issuer_node_id)?.ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "issuer_authorized_node",
        }
        .to_string()
    })?;
    let issuer_grant = store
        .load_grant(&envelope.integration_id, &envelope.issuer_node_id)?
        .ok_or_else(|| {
            IntegrationContractError::Mismatch {
                field: "issuer_integration_node_grant",
            }
            .to_string()
        })?;
    require_active_node_grant(&issuer, &issuer_grant)?;
    let lease = store.load_lease(&envelope.integration_id)?.ok_or_else(|| {
        IntegrationContractError::Mismatch {
            field: "refresh_lease",
        }
        .to_string()
    })?;
    if lease.is_expired(request.now_ms) {
        return Err(IntegrationContractError::ExpiredLease.to_string());
    }
    if lease.holder_node_id != envelope.issuer_node_id {
        return Err(IntegrationContractError::LeaseHolderMismatch.to_string());
    }
    if lease.credential_generation != envelope.credential_generation {
        return Err(IntegrationContractError::LeaseGenerationMismatch.to_string());
    }
    if lease.fencing_token != envelope.refresh_fencing_token {
        return Err(IntegrationContractError::LeaseFenceMismatch.to_string());
    }
    if store
        .max_generation(&envelope.integration_id)?
        .is_some_and(|v| envelope.credential_generation < v)
    {
        return Err(IntegrationContractError::NonMonotonic {
            field: "credential_generation",
        }
        .to_string());
    }
    if let Some(current) = store.load_envelope(
        &envelope.integration_id,
        &envelope.recipient_node_id,
        envelope.credential_generation,
    )? {
        if !(ark_domain::hlc::HLC::is_newer(&envelope.hlc, &current.hlc)
            || (envelope.hlc == current.hlc && envelope.issuer_node_id > current.issuer_node_id))
        {
            return Err(IntegrationContractError::NonMonotonic { field: "hlc" }.to_string());
        }
    }
    store.put_envelope(envelope, &request.device_id, request.now_ms)
}
