use super::*;

pub(super) async fn handle_call(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    call: CallMessage,
) {
    let (grant, broker, integration, stdin) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
            return;
        }
        worker.in_flight += 1;
        (
            worker.grant.clone(),
            worker.broker.clone(),
            worker
                .launch_spec
                .as_ref()
                .and_then(|spec| spec.integration.as_ref())
                .map(|config| config.manifest.clone()),
            worker.stdin.clone(),
        )
    };
    let result = match grant {
        Some(grant) => time::timeout(
            PackageWorkerSupervisor::timeout(),
            dispatch(&inner, &grant, &broker, integration.as_ref(), &call),
        )
        .await
        .unwrap_or_else(|_| Err("timeout")),
        None => Err("forbidden"),
    };
    if let Err(error) = result.as_ref() {
        tracing::warn!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            operation = ?call.operation,
            error,
            "worker call failed"
        );
    }
    if let Some(stdin) = stdin {
        let mut response = match result {
            Ok(value) => result_line(&call.id, true, Some(value), None),
            Err(error) => result_line(&call.id, false, None, Some(error)),
        };
        if response.len() > MAX_LINE_BYTES {
            response = result_line(&call.id, false, None, Some("unavailable"));
        }
        let current = lock(&inner.workers).get(&key).is_some_and(|worker| {
            worker.health.state == WorkerState::Running && worker.generation == call.generation
        });
        if current {
            let _ = stdin.send(response);
        }
    }
    if let Some(worker) = lock(&inner.workers).get_mut(&key) {
        worker.in_flight = worker.in_flight.saturating_sub(1);
    }
}

pub(super) async fn dispatch(
    inner: &SupervisorInner,
    grant: &Grant,
    broker: &BrokerConfig,
    integration: Option<&IntegrationManifest>,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    let store = lock(&inner.store).clone().ok_or("unavailable")?;
    let installed = store
        .installed(&grant.package_id, &grant.version)
        .map_err(|_| "forbidden")?;
    if !installed.enabled || installed.revoked || !installed.hash.eq_ignore_ascii_case(&grant.hash)
    {
        return Err("forbidden");
    }
    if matches!(
        call.operation,
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite
    ) {
        let typed_key = (grant.package_id.clone(), grant.version.clone());
        let typed_bound = lock(&inner.typed_launches).contains_key(&typed_key);
        let typed_envelope = call
            .params
            .get("request")
            .is_some_and(|value| value.get("kind").is_some());
        if typed_envelope {
            if !typed_bound {
                return Err("forbidden");
            }
            if !grant.authenticate(
                &call.token,
                grant.pid,
                call.generation,
                inner.api_major,
                inner.api_major,
            ) {
                return Err("forbidden");
            }
            return dispatch_typed_inner(
                inner,
                &grant.package_id,
                &grant.version,
                &grant.correlation_id,
                call.generation,
                call.params
                    .get("request")
                    .cloned()
                    .unwrap_or_else(|| call.params.clone()),
            )
            .await;
        }
    }
    let scope = match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => call
            .params
            .get("operation")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        WorkerMethod::NetworkFetch => call
            .params
            .get("url")
            .and_then(serde_json::Value::as_str)
            .and_then(|url| reqwest::Url::parse(url).ok())
            .map(|url| url.origin().ascii_serialization()),
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemWrite
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll
        | WorkerMethod::FilesystemDelete => call
            .params
            .get("path")
            .and_then(serde_json::Value::as_str)
            .and_then(|path| granted_path_scope(grant, &call.operation, Path::new(path))),
    };
    if !grant.authorize(
        &call.token,
        grant.pid,
        call.generation,
        inner.api_major,
        inner.api_major,
        &call.operation,
        scope.as_deref(),
    ) {
        return Err("forbidden");
    }
    match call.operation {
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => {
            let operation = scope.ok_or("invalid-request")?;
            let params = call
                .params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let ark = inner.ark.as_ref().ok_or("unavailable")?;
            let response = ark
                .request(&operation, params)
                .await
                .map_err(|_| "unavailable")?;
            if response.ok {
                Ok(response.data)
            } else {
                Err("unavailable")
            }
        }
        WorkerMethod::NetworkFetch => {
            let url = call
                .params
                .get("url")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
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
                        let denied =
                            || package_worker_broker::BrokerError::Invalid("forbidden".into());
                        let integration = integration.ok_or_else(denied)?;
                        let setting = integration
                            .settings
                            .iter()
                            .find(|setting| setting.key == setting_key)
                            .ok_or_else(denied)?;
                        let injection = setting.injection.as_ref().ok_or_else(denied)?;
                        let cookie_names = integration
                            .login
                            .as_ref()
                            .filter(|login| login.secret_setting == setting_key)
                            .map(|login| login.allowed_cookie_names.as_slice())
                            .unwrap_or_default();
                        package_worker_broker::fetch_with_secret_json(
                            broker,
                            url,
                            Some(package_worker_broker::SecretRequest {
                                injection,
                                secret: &secret,
                                allowed_cookie_names: cookie_names,
                            }),
                            call.params.get("body"),
                        )
                        .await
                    }
                    .await;
                    crate::package_worker_secrets::zeroize_secret(&mut secret);
                    response
                }
                None if call.params.get("body").is_none() => {
                    package_worker_broker::fetch(broker, url).await
                }
                None => Err(package_worker_broker::BrokerError::Invalid(
                    "request body requires manifest policy".into(),
                )),
            }
            .map_err(|_| "unavailable")?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemRead => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = package_worker_broker::read_file(broker, Path::new(path)).map_err(|error| {
                let class = match error {
                    package_worker_broker::BrokerError::Invalid(_) => "invalid",
                    package_worker_broker::BrokerError::Io(_) => "io",
                    package_worker_broker::BrokerError::Http(_) => "http",
                };
                tracing::warn!(target: "package_worker", error_class = class, "worker filesystem read failed");
                "unavailable"
            })?;
            if bytes.len() > 700 * 1024 {
                return Err("unavailable");
            }
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemWrite => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let bytes = call
                .params
                .get("bytes")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
                .ok_or("invalid-request")?;
            package_worker_broker::write_file(broker, Path::new(path), &bytes)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemDelete => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            package_worker_broker::delete_file(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemList => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::list_directory(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
        WorkerMethod::FilesystemPoll => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let entries = package_worker_broker::poll_metadata(broker, Path::new(path))
                .map_err(|_| "unavailable")?;
            serde_json::to_value(entries).map_err(|_| "unavailable")
        }
    }
}

pub(super) fn granted_path_scope(
    grant: &Grant,
    method: &WorkerMethod,
    path: &Path,
) -> Option<String> {
    let capability = match method {
        WorkerMethod::FilesystemRead
        | WorkerMethod::FilesystemList
        | WorkerMethod::FilesystemPoll => "filesystem.read",
        WorkerMethod::FilesystemWrite | WorkerMethod::FilesystemDelete => "filesystem.write",
        _ => return None,
    };
    grant
        .scopes
        .get(capability)?
        .iter()
        .find(|root| path.starts_with(root))
        .cloned()
}
