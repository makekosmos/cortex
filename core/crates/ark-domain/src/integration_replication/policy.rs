use std::collections::BTreeSet;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    AuthorizedNode, GrantStatus, IntegrationContractError, IntegrationCredentialEnvelope,
    IntegrationNodeGrant, IntegrationRefreshLease, NodeStatus,
};

pub(crate) fn require_text(
    value: &str,
    field: &'static str,
) -> Result<(), IntegrationContractError> {
    if value.trim().is_empty() {
        return Err(IntegrationContractError::EmptyField { field });
    }
    Ok(())
}

// Key records bind by exact bytes; reject obvious private material and text
// that could be silently normalized. This is not cryptographic key parsing.
pub(crate) fn require_public_key_record(
    value: &str,
    field: &'static str,
) -> Result<(), IntegrationContractError> {
    require_text(value, field)?;
    let normalized = value.to_ascii_lowercase();
    if value != value.trim()
        || value.chars().any(char::is_control)
        || normalized.contains("private key")
        || normalized.contains("begin private")
        || normalized.contains("end private")
    {
        return Err(IntegrationContractError::InvalidPublicKey { field });
    }
    Ok(())
}

pub(crate) fn validate_key_id(value: &str) -> Result<(), IntegrationContractError> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(IntegrationContractError::InvalidKeyId);
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(IntegrationContractError::InvalidKeyId);
    }
    Ok(())
}

pub(crate) fn require_nonzero(
    value: u64,
    field: &'static str,
) -> Result<(), IntegrationContractError> {
    if value == 0 {
        return Err(IntegrationContractError::ZeroValue { field });
    }
    Ok(())
}

pub fn validate_public_settings(value: &Value) -> Result<(), IntegrationContractError> {
    if !value.is_object() {
        return Err(IntegrationContractError::PublicSettingsMustBeObject);
    }
    validate_plaintext_free_payload(value)
}

pub fn validate_plaintext_free_payload(value: &Value) -> Result<(), IntegrationContractError> {
    let mut path = Vec::new();
    reject_sensitive_fields(value, &mut path)
}

/// Stable identifier for the exact serialized recipient encryption public key.
pub fn encryption_key_id(encryption_public_key: &str) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(encryption_public_key.as_bytes())
    )
}

pub fn validate_envelope_recipient(
    envelope: &IntegrationCredentialEnvelope,
    node: &AuthorizedNode,
    grant: &IntegrationNodeGrant,
) -> Result<(), IntegrationContractError> {
    envelope.validate()?;
    node.validate()?;
    grant.validate()?;
    if node.status == NodeStatus::Revoked {
        return Err(IntegrationContractError::RevokedNode);
    }
    if grant.status == GrantStatus::Revoked {
        return Err(IntegrationContractError::RevokedGrant);
    }
    if envelope.integration_id != grant.integration_id {
        return Err(IntegrationContractError::Mismatch {
            field: "integration_id",
        });
    }
    if envelope.recipient_node_id != node.node_id || envelope.recipient_node_id != grant.node_id {
        return Err(IntegrationContractError::Mismatch {
            field: "recipient_node_id",
        });
    }
    if envelope.grant_epoch != node.grant_epoch || envelope.grant_epoch != grant.grant_epoch {
        return Err(IntegrationContractError::Mismatch {
            field: "grant_epoch",
        });
    }
    if grant.node_encryption_key != node.encryption_public_key {
        return Err(IntegrationContractError::Mismatch {
            field: "node_encryption_key",
        });
    }
    if envelope.key_id != encryption_key_id(&node.encryption_public_key) {
        return Err(IntegrationContractError::Mismatch { field: "key_id" });
    }
    Ok(())
}

pub fn require_newer(
    proposed: u64,
    current: u64,
    field: &'static str,
) -> Result<u64, IntegrationContractError> {
    if proposed <= current {
        return Err(IntegrationContractError::NonMonotonic { field });
    }
    Ok(proposed)
}

pub fn next_credential_generation(
    current: u64,
    proposed: u64,
) -> Result<u64, IntegrationContractError> {
    require_newer(proposed, current, "credential_generation")
}

pub fn acquire_refresh_lease(
    current: Option<&IntegrationRefreshLease>,
    integration_id: impl Into<String>,
    holder_node_id: impl Into<String>,
    credential_generation: u64,
    now_ms: u64,
    ttl_ms: u64,
) -> Result<IntegrationRefreshLease, IntegrationContractError> {
    let integration_id = integration_id.into();
    let holder_node_id = holder_node_id.into();
    require_text(&integration_id, "integration_id")?;
    require_text(&holder_node_id, "holder_node_id")?;
    require_nonzero(credential_generation, "credential_generation")?;
    require_nonzero(ttl_ms, "ttl_ms")?;
    if current.is_some_and(|lease| !lease.is_expired(now_ms)) {
        return Err(IntegrationContractError::Mismatch {
            field: "active_lease",
        });
    }
    let fencing_token = current.map_or(1, |lease| lease.fencing_token.saturating_add(1));
    let lease = IntegrationRefreshLease {
        integration_id,
        credential_generation,
        holder_node_id,
        fencing_token,
        issued_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(ttl_ms),
    };
    lease.validate()?;
    Ok(lease)
}

pub fn validate_refresh_publication(
    envelope: &IntegrationCredentialEnvelope,
    lease: &IntegrationRefreshLease,
    node: &AuthorizedNode,
    grant: &IntegrationNodeGrant,
    current_generation: u64,
    now_ms: u64,
) -> Result<u64, IntegrationContractError> {
    validate_envelope_recipient(envelope, node, grant)?;
    lease.validate()?;
    if lease.is_expired(now_ms) {
        return Err(IntegrationContractError::ExpiredLease);
    }
    if envelope.integration_id != lease.integration_id {
        return Err(IntegrationContractError::Mismatch {
            field: "integration_id",
        });
    }
    if envelope.issuer_node_id != lease.holder_node_id {
        return Err(IntegrationContractError::LeaseHolderMismatch);
    }
    if envelope.credential_generation != lease.credential_generation {
        return Err(IntegrationContractError::LeaseGenerationMismatch);
    }
    if envelope.refresh_fencing_token != lease.fencing_token {
        return Err(IntegrationContractError::LeaseFenceMismatch);
    }
    next_credential_generation(current_generation, envelope.credential_generation)
}

fn reject_sensitive_fields(
    value: &Value,
    path: &mut Vec<String>,
) -> Result<(), IntegrationContractError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                path.push(key.clone());
                if is_sensitive_key(key) {
                    return Err(IntegrationContractError::SensitiveField {
                        path: format_json_path(path),
                    });
                }
                reject_sensitive_fields(child, path)?;
                path.pop();
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                path.push(index.to_string());
                reject_sensitive_fields(child, path)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let fragments = BTreeSet::from([
        "accesskey",
        "accesstoken",
        "apikey",
        "authorization",
        "authtoken",
        "bearer",
        "clientsecret",
        "credential",
        "passphrase",
        "password",
        "privatekey",
        "refreshtoken",
        "secret",
        "token",
    ]);
    fragments
        .iter()
        .any(|fragment| normalized.contains(fragment))
}

fn format_json_path(path: &[String]) -> String {
    path.iter().fold(String::from("$"), |mut output, segment| {
        if segment.chars().all(|character| character.is_ascii_digit()) {
            output.push('[');
            output.push_str(segment);
            output.push(']');
        } else {
            output.push('.');
            output.push_str(segment);
        }
        output
    })
}
