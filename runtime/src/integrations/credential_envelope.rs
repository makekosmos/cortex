use super::{json, Value};
use crate::ark_host::ArkHost;
use crate::package_service::PackageService;

pub(crate) async fn receive(
    params: &Value,
    ark: &ArkHost,
    packages: &PackageService,
) -> Result<Value, String> {
    let space_id = required(params, "space_id")?;
    let integration_id = required(params, "integration_id")?;
    let recipient_node_id = required(params, "recipient_node_id")?;
    let issuer_node_id = required(params, "issuer_node_id")?;
    let credential_generation = params
        .get("credential_generation")
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or("credential_generation is required")?;
    let expected_issuer_key_id = required(params, "issuer_auth_key_id")?;
    let consumer = super::replication_consumer::ReplicationConsumer::new(ark);
    let issuer = consumer
        .lookup_issuer_encryption_key(&json!({
            "space_id": space_id,
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
            "issuer_node_id": issuer_node_id,
            "credential_generation": credential_generation,
            "expected_issuer_key_id": expected_issuer_key_id,
        }))
        .await
        .map_err(|_| "Core issuer key lookup rejected".to_string())?;
    if issuer.get("schema_version").and_then(Value::as_u64) != Some(1)
        || issuer.get("space_id").and_then(Value::as_str) != Some(space_id)
        || issuer.get("integration_id").and_then(Value::as_str) != Some(integration_id)
        || issuer.get("recipient_node_id").and_then(Value::as_str) != Some(recipient_node_id)
        || issuer.get("issuer_node_id").and_then(Value::as_str) != Some(issuer_node_id)
        || issuer.get("credential_generation").and_then(Value::as_u64)
            != Some(credential_generation)
        || issuer.get("status").and_then(Value::as_str) != Some("active")
        || issuer.get("grant_status").and_then(Value::as_str) != Some("active")
    {
        return Err("Core issuer key lookup binding mismatch".to_string());
    }
    let issuer_key_id = required(&issuer, "key_id")?;
    if issuer_key_id != expected_issuer_key_id {
        return Err("Core issuer key lookup key id mismatch".to_string());
    }
    let grant_epoch = issuer
        .get("grant_epoch")
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or("Core issuer key lookup returned no grant epoch")?;
    let refresh_fencing_token = issuer
        .get("refresh_fencing_token")
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or("Core issuer key lookup returned no refresh fence")?;
    let loaded = consumer
        .load_latest_credential_envelope(&json!({
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
        }))
        .await
        .map_err(|_| "Core credential envelope load rejected".to_string())?;
    let (package_version, setting) = packages
        .integration_credential_target(integration_id)
        .map_err(|_| "installed integration manifest unavailable".to_string())?;
    let recipient =
        crate::package_service::credential_envelope::load_or_create_identity(recipient_node_id)
            .map_err(|_| "local credential envelope identity unavailable".to_string())?;
    let expected = crate::package_service::credential_envelope::CredentialContext {
        integration_id: integration_id.to_owned(),
        recipient_node_id: recipient_node_id.to_owned(),
        grant_epoch,
        credential_generation,
        key_id: recipient.key_id.clone(),
        issuer_node_id: issuer_node_id.to_owned(),
        issuer_auth_key_id: expected_issuer_key_id.to_owned(),
    };
    let envelope =
        crate::package_service::credential_envelope::CredentialEnvelopeV2::from_core_value(
            &loaded,
            expected_issuer_key_id,
        )
        .map_err(|_| "Core credential envelope is not V2 HPKE".to_string())?;
    let confirmed = consumer
        .lookup_issuer_encryption_key(&json!({
            "space_id": space_id,
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
            "issuer_node_id": issuer_node_id,
            "credential_generation": credential_generation,
            "expected_issuer_key_id": expected_issuer_key_id,
        }))
        .await
        .map_err(|_| "Core issuer key revalidation rejected".to_string())?;
    if confirmed.get("status").and_then(Value::as_str) != Some("active")
        || confirmed.get("grant_status").and_then(Value::as_str) != Some("active")
        || confirmed.get("key_id").and_then(Value::as_str) != Some(expected_issuer_key_id)
        || confirmed.get("grant_epoch").and_then(Value::as_u64) != Some(grant_epoch)
        || confirmed
            .get("credential_generation")
            .and_then(Value::as_u64)
            != Some(credential_generation)
    {
        return Err("Core issuer key changed during credential load".to_string());
    }
    let issuer_public_key = required(&confirmed, "encryption_public_key")?;
    let plaintext = crate::package_service::credential_envelope::decrypt(
        &envelope,
        &expected,
        &recipient,
        issuer_public_key,
    )
    .map_err(|_| "credential envelope verification failed".to_string())?;
    let final_state = consumer
        .lookup_issuer_encryption_key(&json!({
            "space_id": space_id,
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
            "issuer_node_id": issuer_node_id,
            "credential_generation": credential_generation,
            "expected_issuer_key_id": expected_issuer_key_id,
        }))
        .await
        .map_err(|_| "Core issuer key finalization rejected".to_string())?;
    if final_state.get("status").and_then(Value::as_str) != Some("active")
        || final_state.get("grant_status").and_then(Value::as_str) != Some("active")
        || final_state.get("key_id").and_then(Value::as_str) != Some(expected_issuer_key_id)
        || final_state.get("grant_epoch").and_then(Value::as_u64) != Some(grant_epoch)
        || final_state
            .get("credential_generation")
            .and_then(Value::as_u64)
            != Some(credential_generation)
    {
        return Err("Core issuer key changed before credential storage".to_string());
    }
    consumer
        .check_credential_fence(&json!({
            "space_id": space_id,
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
            "issuer_node_id": issuer_node_id,
            "credential_generation": credential_generation,
            "refresh_fencing_token": refresh_fencing_token,
            "expected_issuer_key_id": expected_issuer_key_id,
        }))
        .await
        .map_err(|_| "Core credential fence rejected".to_string())?;
    let stored = packages
        .store_integration_credential_and_sync(
            integration_id,
            &package_version,
            &setting,
            &plaintext,
        )
        .await
        .map_err(|_| "stored credential could not start provider sync".to_string());
    stored?;
    Ok(json!({
        "stored": true,
        "version": crate::package_service::credential_envelope::ENVELOPE_VERSION
    }))
}

pub(crate) async fn publish(
    params: &Value,
    ark: &ArkHost,
    packages: &crate::package_service::PackageService,
) -> Result<Value, String> {
    let space_id = required(params, "space_id")?;
    let integration_id = required(params, "integration_id")?;
    let issuer_node_id = required(params, "issuer_node_id")?;
    let recipient_node_id = required(params, "recipient_node_id")?;
    let recipient_public_key = required(params, "recipient_public_key")?;
    let grant_epoch = nonzero(params, "grant_epoch")?;
    let credential_generation = nonzero(params, "credential_generation")?;
    let refresh_fencing_token = nonzero(params, "refresh_fencing_token")?;
    let (package_version, setting) = packages
        .integration_credential_target(integration_id)
        .map_err(|_| "installed integration manifest unavailable".to_string())?;
    let issuer =
        crate::package_service::credential_envelope::load_or_create_identity(issuer_node_id)
            .map_err(|_| "local credential envelope identity unavailable".to_string())?;
    let secret = crate::package_service::secret_store::read_package_integration_secret(
        integration_id,
        &package_version,
        &setting,
    )
    .ok_or("credential is not stored locally")?;
    let lookup = super::replication_consumer::ReplicationConsumer::new(ark)
        .lookup_issuer_encryption_key_for_publish(&json!({
            "space_id": space_id,
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
            "issuer_node_id": issuer_node_id,
            "expected_issuer_key_id": issuer.key_id,
        }))
        .await
        .map_err(|_| "Core issuer key lookup rejected".to_string())?;
    if lookup.get("key_id").and_then(Value::as_str) != Some(issuer.key_id.as_str())
        || lookup.get("encryption_public_key").and_then(Value::as_str)
            != Some(issuer.public_key.as_str())
        || lookup.get("status").and_then(Value::as_str) != Some("active")
        || lookup.get("grant_status").and_then(Value::as_str) != Some("active")
    {
        return Err("local issuer key is not the authorized Core key".to_string());
    }
    let context = crate::package_service::credential_envelope::CredentialContext {
        integration_id: integration_id.to_owned(),
        recipient_node_id: recipient_node_id.to_owned(),
        grant_epoch,
        credential_generation,
        key_id: key_id(recipient_public_key),
        issuer_node_id: issuer_node_id.to_owned(),
        issuer_auth_key_id: issuer.key_id.clone(),
    };
    let encrypted = crate::package_service::credential_envelope::encrypt(
        &context,
        recipient_public_key,
        &issuer,
        &secret,
    )
    .map_err(|_| "credential encryption failed".to_string())?;
    let envelope = json!({
        "integration_id": encrypted.integration_id,
        "recipient_node_id": encrypted.recipient_node_id,
        "grant_epoch": encrypted.grant_epoch,
        "credential_generation": encrypted.credential_generation,
        "refresh_fencing_token": refresh_fencing_token,
        "key_id": encrypted.key_id,
        "algorithm": encrypted.algorithm,
        "nonce": encrypted.enc,
        "ciphertext": encrypted.ciphertext,
        "issuer_node_id": encrypted.issuer_node_id,
        "issued_at": required(params, "issued_at")?,
        "revision": nonzero(params, "revision")?,
        "hlc": required(params, "hlc")?,
    });
    let response = super::replication_consumer::ReplicationConsumer::new(ark)
        .publish_credential_envelope(&json!({
            "envelope": envelope,
            "device_id": required(params, "device_id")?,
            "now_ms": nonzero(params, "now_ms")?,
        }))
        .await
        .map_err(|_| "Core credential publication rejected".to_string())?;
    Ok(
        json!({ "published": response, "version": crate::package_service::credential_envelope::ENVELOPE_VERSION }),
    )
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} is required"))
}

fn nonzero(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or_else(|| format!("{field} is required"))
}

fn key_id(public_key: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(public_key.as_bytes()))
}
