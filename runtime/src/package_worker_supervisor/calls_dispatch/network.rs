use super::*;

pub(super) async fn dispatch_network(
    inner: &SupervisorInner,
    grant: &Grant,
    broker: &BrokerConfig,
    integration: Option<&IntegrationManifest>,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    let url = call
        .params
        .get("url")
        .and_then(serde_json::Value::as_str)
        .ok_or("invalid-request")?;
    let owner = network_owner(&grant.package_id, &grant.version, call.generation);
    if let Some(handle) = call.params.get("response_handle") {
        let handle = handle.as_str().ok_or("invalid-request")?;
        if call.params.get("close") == Some(&serde_json::Value::Bool(true)) {
            inner
                .network_responses
                .close(handle, &owner)
                .map_err(|_| "forbidden")?;
            return Ok(serde_json::Value::Null);
        }
        let offset = call
            .params
            .get("offset")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("invalid-request")?;
        let length = call
            .params
            .get("length")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("invalid-request")?;
        if length == 0 {
            return Err("invalid-request");
        }
        let bytes = inner
            .network_responses
            .chunk(handle, &owner, offset, length)
            .map_err(|_| "forbidden")?;
        return Ok(
            serde_json::json!({"bytes": base64::engine::general_purpose::STANDARD.encode(bytes)}),
        );
    }
    let chunked = match call
        .params
        .get("response_mode")
        .and_then(serde_json::Value::as_str)
    {
        None | Some("inline") => false,
        Some("chunks") => true,
        _ => return Err("invalid-request"),
    };
    // Bound in-flight large downloads as well as retained response buffers.
    let _slot = if chunked {
        Some(
            inner
                .network_slots
                .acquire()
                .await
                .map_err(|_| "unavailable")?,
        )
    } else {
        None
    };
    let limit = if chunked {
        package_worker_broker::MAX_NETWORK_RESPONSE
    } else {
        1024 * 1024
    };
    let secret_handle = call
        .params
        .get("secret_handle")
        .and_then(serde_json::Value::as_str);
    let bytes = match secret_handle {
        Some(handle) => {
            let (setting_key, mut secret) = inner
                .secrets
                .resolve_token(handle, &grant.package_id, &grant.version, call.generation)
                .map_err(|_| "forbidden")?;
            let response = async {
                let denied = || package_worker_broker::BrokerError::Invalid("forbidden".into());
                let integration = integration.ok_or_else(denied)?;
                let setting = integration
                    .settings
                    .iter()
                    .find(|setting| setting.key == setting_key)
                    .ok_or_else(denied)?;
                let injection = setting.injection.as_ref().ok_or_else(denied)?;
                if integration.login.as_ref().is_some_and(|login| {
                    login.secret_setting == setting_key
                        && login.code_exchange.as_deref() == Some("huawei_health")
                }) {
                    let updated = crate::package_service::huawei_login::session_for_request(
                        &grant.package_id,
                        &grant.version,
                        &setting_key,
                        &secret,
                    )
                    .await
                    .map_err(|_| denied())?;
                    crate::package_worker_secrets::zeroize_secret(&mut secret);
                    secret = updated;
                }
                let cookie_names = integration
                    .login
                    .as_ref()
                    .filter(|login| login.secret_setting == setting_key)
                    .map(|login| login.allowed_cookie_names.as_slice())
                    .unwrap_or_default();
                package_worker_broker::fetch_with_secret_json_limit(
                    broker,
                    url,
                    Some(package_worker_broker::SecretRequest {
                        injection,
                        secret: &secret,
                        allowed_cookie_names: cookie_names,
                    }),
                    call.params.get("body"),
                    limit,
                )
                .await
            }
            .await;
            crate::package_worker_secrets::zeroize_secret(&mut secret);
            response
        }
        None if call.params.get("body").is_none() => {
            package_worker_broker::fetch_with_secret_json_limit(broker, url, None, None, limit)
                .await
        }
        None => Err(package_worker_broker::BrokerError::Invalid(
            "request body requires manifest policy".into(),
        )),
    }
    .map_err(|_| "unavailable")?;
    if chunked {
        let workers = lock(&inner.workers);
        let live = workers
            .get(&(grant.package_id.clone(), grant.version.clone()))
            .filter(|worker| {
                worker.generation == call.generation
                    && worker.health.state == WorkerState::Running
                    && !worker.cleanup_started
                    && worker.grant.is_some()
            });
        if live.is_none() {
            return Err("forbidden");
        }
        let size = bytes.len();
        let handle = inner
            .network_responses
            .reserve(&owner, &grant.package_id, url, bytes)
            .map_err(|_| "unavailable")?;
        return Ok(serde_json::json!({"response_handle": handle, "size": size,
            "chunk_size": package_worker_broker::MAX_SNAPSHOT_CHUNK}));
    }
    if bytes.len() > 700 * 1024 {
        return Err("unavailable");
    }
    Ok(serde_json::json!({
        "bytes": base64::engine::general_purpose::STANDARD.encode(bytes)
    }))
}
