use super::*;

pub(super) fn try_dispatch_handle(
    inner: &SupervisorInner,
    grant: &Grant,
    call: &CallMessage,
) -> Option<Result<serde_json::Value, &'static str>> {
    if call.operation == WorkerMethod::FilesystemRootOpen {
        return Some(open_root(inner, grant, call));
    }
    matches!(
        call.operation,
        WorkerMethod::FilesystemRead
            | WorkerMethod::FilesystemWrite
            | WorkerMethod::FilesystemList
            | WorkerMethod::FilesystemDelete
            | WorkerMethod::FilesystemCreateDir
    )
    .then(|| {
        call.params
            .get("root_id")
            .map(|_| dispatch(inner, grant, call))
    })
    .flatten()
}

fn authenticate(inner: &SupervisorInner, grant: &Grant, call: &CallMessage) -> bool {
    grant.authenticate(
        &call.token,
        grant.pid,
        call.generation,
        inner.api_major,
        inner.api_major,
    )
}

fn owner(grant: &Grant, generation: u64) -> GrantOwner {
    GrantOwner {
        session_id: format!("{}:{}", grant.correlation_id, grant.package_id),
        generation,
        connection_id: u64::from(grant.pid),
    }
}

fn open_root(
    inner: &SupervisorInner,
    grant: &Grant,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    if !authenticate(inner, grant, call)
        || !(grant.scopes.contains_key("filesystem.read")
            || grant.scopes.contains_key("filesystem.write"))
    {
        return Err("forbidden");
    }
    let persistent_id = call
        .params
        .get("persistent_grant_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .ok_or("invalid-request")?;
    let root_id = lock(&inner.grants)
        .clone()
        .ok_or("unavailable")?
        .reopen(
            &owner(grant, call.generation),
            persistent_id,
            &grant.package_id,
        )
        .map_err(|_| "forbidden")?
        .0;
    Ok(serde_json::json!({ "root_id": root_id }))
}

fn dispatch(
    inner: &SupervisorInner,
    grant: &Grant,
    call: &CallMessage,
) -> Result<serde_json::Value, &'static str> {
    if !authenticate(inner, grant, call) {
        return Err("forbidden");
    }
    let capability = if matches!(
        call.operation,
        WorkerMethod::FilesystemWrite
            | WorkerMethod::FilesystemDelete
            | WorkerMethod::FilesystemCreateDir
    ) {
        "filesystem.write"
    } else {
        "filesystem.read"
    };
    if !grant.scopes.contains_key(capability) {
        return Err("forbidden");
    }
    let root_id = call
        .params
        .get("root_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .ok_or("invalid-request")?;
    let relative = call
        .params
        .get("relative_path")
        .and_then(serde_json::Value::as_str)
        .ok_or("invalid-request")?;
    let components = if relative.is_empty() {
        Vec::new()
    } else {
        relative
            .split('/')
            .map(|component| {
                (!component.is_empty()
                    && component != "."
                    && component != ".."
                    && !component.contains('\\'))
                .then_some(component)
                .ok_or("invalid-request")
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let owner = owner(grant, call.generation);
    let grants = lock(&inner.grants).clone().ok_or("unavailable")?;
    match call.operation {
        WorkerMethod::FilesystemRead => {
            let bytes = grants
                .read(root_id, &owner, &grant.package_id, &components, 700 * 1024)
                .map_err(|_| "unavailable")?;
            Ok(
                serde_json::json!({ "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) }),
            )
        }
        WorkerMethod::FilesystemWrite => {
            let bytes = call
                .params
                .get("bytes")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
                .ok_or("invalid-request")?;
            grants
                .write(root_id, &owner, &grant.package_id, &components, &bytes)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemList => Ok(serde_json::Value::Array(
            grants
                .list(root_id, &owner, &grant.package_id, &components)
                .map_err(|_| "unavailable")?
                .into_iter()
                .map(|entry| {
                    serde_json::json!({
                        "name": entry.name,
                        "kind": if entry.directory { "directory" } else { "file" }
                    })
                })
                .collect(),
        )),
        WorkerMethod::FilesystemDelete => {
            grants
                .delete(root_id, &owner, &grant.package_id, &components)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        WorkerMethod::FilesystemCreateDir => {
            grants
                .mkdir(root_id, &owner, &grant.package_id, &components)
                .map_err(|_| "unavailable")?;
            Ok(serde_json::Value::Null)
        }
        _ => Err("invalid-request"),
    }
}
