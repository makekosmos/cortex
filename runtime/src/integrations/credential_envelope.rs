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
    let issuer_public_key = required(&issuer, "encryption_public_key")?;
    let issuer_key_id = required(&issuer, "key_id")?;
    if issuer_key_id != expected_issuer_key_id {
        return Err("Core issuer key lookup key id mismatch".to_string());
    }
    let grant_epoch = issuer
        .get("grant_epoch")
        .and_then(Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or("Core issuer key lookup returned no grant epoch")?;
    let loaded = consumer
        .load_latest_credential_envelope(&json!({
            "integration_id": integration_id,
            "recipient_node_id": recipient_node_id,
        }))
        .await
        .map_err(|_| "Core credential envelope load rejected".to_string())?;
    let package_version = required(params, "package_version")?;
    let setting = required(params, "setting")?;
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
    crate::package_service::credential_envelope::decrypt_and_store(
        &envelope,
        &expected,
        &recipient,
        issuer_public_key,
        integration_id,
        package_version,
        setting,
    )
    .map_err(|_| "credential envelope verification failed".to_string())?;
    packages
        .sync_integration_now(integration_id)
        .map_err(|_| "stored credential could not start provider sync".to_string())?;
    Ok(json!({
        "stored": true,
        "version": crate::package_service::credential_envelope::ENVELOPE_VERSION
    }))
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{field} is required"))
}
