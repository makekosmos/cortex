use super::*;
#[path = "calls_dispatch/handle.rs"]
mod handle;
#[path = "calls_dispatch/handle_relative.rs"]
mod handle_relative;
#[path = "calls_dispatch/network.rs"]
mod network;
#[path = "calls_dispatch/path_scope.rs"]
mod path_scope;
pub(super) use handle::handle_call;
use {
    handle_relative::try_dispatch_handle, network::dispatch_network, path_scope::granted_path_scope,
};

pub(super) fn network_owner(package: &str, version: &str, generation: u64) -> String {
    serde_json::json!([package, version, generation]).to_string()
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
    if let Some(result) = try_dispatch_handle(inner, grant, call) {
        return result;
    }
    if call.operation == WorkerMethod::ArkRead || call.operation == WorkerMethod::ArkWrite {
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
        WorkerMethod::FilesystemRootOpen => None,
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
        | WorkerMethod::FilesystemDelete
        | WorkerMethod::FilesystemCreateDir
        | WorkerMethod::ProcessSpawn => call
            .params
            .get(if matches!(call.operation, WorkerMethod::ProcessSpawn) {
                "executable"
            } else {
                "path"
            })
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
        WorkerMethod::FilesystemRootOpen => handle_relative::open_root(inner, grant, call),
        WorkerMethod::ArkRead | WorkerMethod::ArkWrite => {
            let operation = scope.ok_or("invalid-request")?;
            let params = call
                .params
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            inner.ark_executor.request(&operation, params).await
        }
        WorkerMethod::NetworkFetch => {
            dispatch_network(inner, grant, broker, integration, call).await
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
                    package_worker_broker::BrokerError::Io(ref error)
                        if error.kind() == std::io::ErrorKind::NotFound => return "not-found",
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
        WorkerMethod::FilesystemCreateDir => {
            let path = call
                .params
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            package_worker_broker::create_directory(broker, Path::new(path))
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
                .map_err(path_scope::filesystem_error)?;
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
        WorkerMethod::ProcessSpawn => {
            let executable = call
                .params
                .get("executable")
                .and_then(serde_json::Value::as_str)
                .ok_or("invalid-request")?;
            let args = call
                .params
                .get("args")
                .and_then(serde_json::Value::as_array)
                .map(|args| {
                    args.iter()
                        .map(|arg| arg.as_str().ok_or("invalid-request"))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            let cwd = call.params.get("cwd").and_then(serde_json::Value::as_str);
            if cwd.is_some_and(|cwd| {
                granted_path_scope(grant, &WorkerMethod::ProcessSpawn, Path::new(cwd)).is_none()
            }) {
                return Err("forbidden");
            }
            let pid = package_worker_broker::spawn_process(
                broker,
                Path::new(executable),
                &args,
                cwd.map(Path::new),
            )
            .map_err(|_| "unavailable")?;
            Ok(serde_json::json!({ "pid": pid }))
        }
    }
}
