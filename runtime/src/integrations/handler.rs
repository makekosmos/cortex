use super::*;

fn provider_id(params: &Value) -> Result<&str, String> {
    params
        .get("provider")
        .and_then(Value::as_str)
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
        .ok_or_else(|| "Не указана интеграция".to_string())
}

fn snapshot(
    data_dir: &Path,
    packages: &crate::package_service::PackageService,
) -> Result<Value, String> {
    Ok(json!({
        "providers": packages
            .integration_provider_snapshots()
            .map_err(|_| "Не удалось прочитать пакетные интеграции".to_string())?,
        "bodyWeightKg": read_config(data_dir).body_weight_kg,
    }))
}

pub async fn handle_operation(
    subop: &str,
    params: Value,
    ark: &ArkHost,
    data_dir: &Path,
    packages: &crate::package_service::PackageService,
) -> Result<Value, String> {
    match subop {
        "replication_authorize_node" | "replication_revoke_node" | "replication_rotate_node" => {
            let operation = match subop {
                "replication_authorize_node" => "authorize",
                "replication_revoke_node" => "revoke",
                "replication_rotate_node" => "rotate",
                _ => return Err("invalid replication operation".into()),
            };
            let node = params.get("node").ok_or("node is required")?;
            if !node.is_object() {
                return Err("node must be an object".into());
            }
            let grant = params.get("grant");
            if grant.is_some_and(|value| !value.is_object()) {
                return Err("grant must be an object".into());
            }
            let device_id = params
                .get("device_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or("device_id is required")?;
            super::replication_consumer::ReplicationConsumer::new(ark)
                .persist_node_authorization(operation, node, grant, device_id)
                .await
                .map(|_| json!({ "accepted": true }))
                .map_err(|_| "Core node authorization rejected".to_string())
        }
        "replication_persist_grant" => {
            let grant = params.get("grant").ok_or("grant is required")?;
            if !grant.is_object() {
                return Err("grant must be an object".into());
            }
            let device_id = params
                .get("device_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or("device_id is required")?;
            super::replication_consumer::ReplicationConsumer::new(ark)
                .persist_integration_grant(grant, device_id)
                .await
                .map(|_| json!({ "accepted": true }))
                .map_err(|_| "Core integration grant rejected".to_string())
        }
        "list" => snapshot(data_dir, packages),
        "login_contract" => packages
            .integration_login_contract(provider_id(&params)?)
            .map_err(|_| "Интеграция не содержит доверенного сценария входа".to_string()),
        "set_credential" => {
            let id = provider_id(&params)?;
            let value = params
                .get("credential")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Укажите данные подключения".to_string())?;
            packages
                .set_integration_value(id, params.get("setting").and_then(Value::as_str), value)
                .await
                .map_err(|_| "Не удалось сохранить настройку интеграции".to_string())?;
            snapshot(data_dir, packages)
        }
        "clear_credential" => {
            packages
                .clear_integration_values(provider_id(&params)?)
                .await
                .map_err(|_| "Не удалось очистить настройки интеграции".to_string())?;
            snapshot(data_dir, packages)
        }
        "sync_now" => {
            packages
                .sync_integration_now(provider_id(&params)?)
                .map_err(|_| "Интеграция недоступна".to_string())?;
            snapshot(data_dir, packages)
        }
        "replication_prepare_signed_sync" => {
            let consumer = super::replication_consumer::ReplicationConsumer::new(ark);
            consumer
                .prepare_signed_sync(
                    params
                        .get("space_id")
                        .and_then(Value::as_str)
                        .ok_or("space_id is required")?,
                    params
                        .get("origin_node_id")
                        .and_then(Value::as_str)
                        .ok_or("origin_node_id is required")?,
                    params
                        .get("integration_id")
                        .and_then(Value::as_str)
                        .ok_or("integration_id is required")?,
                    params
                        .get("recipient_node_id")
                        .and_then(Value::as_str)
                        .ok_or("recipient_node_id is required")?,
                    params
                        .get("message_id")
                        .and_then(Value::as_str)
                        .ok_or("message_id is required")?,
                )
                .await
                .map_err(|_| "Не удалось подготовить репликацию".to_string())
        }
        "replication_validate_outbound_signed_sync" => {
            let consumer = super::replication_consumer::ReplicationConsumer::new(ark);
            consumer
                .validate_outbound_signed_sync(
                    params
                        .get("space_id")
                        .and_then(Value::as_str)
                        .ok_or("space_id is required")?,
                    params
                        .get("origin_node_id")
                        .and_then(Value::as_str)
                        .ok_or("origin_node_id is required")?,
                    params.get("frame").ok_or("frame is required")?,
                )
                .await
                .map(|accepted| json!({ "accepted": accepted }))
                .map_err(|_| "Исходящий кадр репликации отклонён".to_string())
        }
        "replication_send_signed_sync" => {
            let frame = params.get("frame").ok_or("frame is required")?;
            if !frame.is_object() {
                return Err("frame must be an object".into());
            }
            super::replication_consumer::ReplicationConsumer::new(ark)
                .send_signed_sync(frame)
                .await
                .map(|sent| json!({ "sent": sent }))
                .map_err(|_| "Адресная отправка репликации отклонена".to_string())
        }
        "replication_acquire_refresh_lease"
        | "replication_load_latest_credential_envelope"
        | "replication_verification_status" => {
            for field in match subop {
                "replication_acquire_refresh_lease" => [
                    "integration_id",
                    "holder_node_id",
                    "credential_generation",
                    "now_ms",
                    "ttl_ms",
                    "expected_fencing_token",
                    "device_id",
                ]
                .as_slice(),
                "replication_load_latest_credential_envelope" => {
                    ["integration_id", "recipient_node_id"].as_slice()
                }
                _ => ["integration_id", "local_node_id", "now_ms"].as_slice(),
            } {
                if params.get(field).is_none() {
                    return Err(format!("{field} is required"));
                }
            }
            let consumer = super::replication_consumer::ReplicationConsumer::new(ark);
            let operation = match subop {
                "replication_acquire_refresh_lease" => {
                    consumer.acquire_refresh_lease(&params).await
                }
                "replication_load_latest_credential_envelope" => {
                    consumer.load_latest_credential_envelope(&params).await
                }
                _ => consumer.verification_status(&params).await,
            };
            operation.map_err(|_| "Core refresh operation rejected".to_string())
        }
        "replication_publish_credential_envelope" => {
            let envelope = params.get("envelope").ok_or("envelope is required")?;
            if !envelope.is_object() {
                return Err("envelope must be an object".into());
            }
            if params
                .get("device_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .is_none()
            {
                return Err("device_id is required".into());
            }
            params
                .get("now_ms")
                .and_then(Value::as_u64)
                .ok_or("now_ms is required")?;
            super::replication_consumer::ReplicationConsumer::new(ark)
                .publish_credential_envelope(&params)
                .await
                .map(|published| json!({ "published": published }))
                .map_err(|_| "Core credential publication rejected".to_string())
        }
        "replication_receive_credential_envelope_v2" => {
            super::credential_envelope::receive(&params, ark, packages).await
        }
        "body_weight_set" => {
            let body_weight = params.get("bodyWeightKg").and_then(Value::as_f64);
            if body_weight.is_some_and(|value| !(20.0..=400.0).contains(&value)) {
                return Err("Вес тела должен быть от 20 до 400 кг".to_string());
            }
            mutate_config(data_dir, |config| {
                config.body_weight_kg = body_weight;
                Ok(())
            })?;
            snapshot(data_dir, packages)
        }
        "body_snapshot" => {
            body_snapshot(
                ark,
                data_dir,
                params
                    .get("range")
                    .and_then(Value::as_str)
                    .unwrap_or("week"),
            )
            .await
        }
        _ => Err(format!("Неизвестная операция integrations.{subop}")),
    }
}
