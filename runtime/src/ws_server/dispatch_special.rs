use super::*;
pub(super) async fn dispatch_special(
    request: crate::engine_dispatch::DispatchRequest,
    package_service: Arc<PackageService>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    client_id: ClientId,
) -> LocalResponse {
    let operation = request.operation.as_str().to_owned();
    let params = request.params;
    let connection_id = client_id;
    if !(operation.starts_with("package.snapshot.") || operation.starts_with("grant.")) {
        return LocalResponse::err("not a special operation");
    }
    let response = {
        if !request.client.desktop_authorized {
            LocalResponse::err("desktop authority denied")
        } else {
            let snapshots = snapshots;
            let owner = format!("desktop-connection-{connection_id}");
            match operation.as_str() {
                "package.snapshot.reserve" => {
                    let package_id = params.get("packageId").and_then(Value::as_str);
                    let source = params.get("source").and_then(Value::as_str);
                    match (package_id, source) {
                        (Some(package_id), Some(source)) => {
                            let build = package_service.clone();
                            let package_id = package_id.to_owned();
                            let source = source.to_owned();
                            let build_package_id = package_id.clone();
                            let build_source = source.clone();
                            match tokio::task::spawn_blocking(move || {
                                build.build_engine_snapshot(&build_package_id, &build_source)
                            })
                            .await
                            {
                                Ok(Ok(files)) => {
                                    let (root_realpath, root_dev, root_ino) = match package_service
                                        .engine_snapshot_identity(&package_id, &source)
                                    {
                                        Ok(value) => value,
                                        Err(error) => {
                                            return LocalResponse::err(error.to_string());
                                        }
                                    };
                                    match serde_json::to_vec(&files) {
                                        Ok(bytes) => match snapshots.reserve(
                                            &owner,
                                            &package_id,
                                            &source,
                                            bytes,
                                        ) {
                                            Ok(handle) => match snapshots.size(&handle, &owner) {
                                                Ok(size) => LocalResponse::ok(serde_json::json!({
                                                    "handle": handle,
                                                    "packageId": package_id,
                                                    "source": source,
                                                    "size": size,
                                                    "rootRealpath": root_realpath,
                                                    "rootIdentity": {
                                                        "dev": root_dev,
                                                        "ino": root_ino,
                                                    },
                                                })),
                                                Err(error) => LocalResponse::err(error.to_string()),
                                            },
                                            Err(error) => LocalResponse::err(error.to_string()),
                                        },
                                        Err(error) => LocalResponse::err(error.to_string()),
                                    }
                                }
                                Ok(Err(error)) => LocalResponse::err(error.to_string()),
                                Err(error) => LocalResponse::err(error.to_string()),
                            }
                        }
                        _ => LocalResponse::err("package snapshot requires packageId and source"),
                    }
                }
                "grant.authority.register" => {
                    if !request.client.desktop_authorized {
                        return LocalResponse::err("grant authority denied");
                    }
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return LocalResponse::err("grant authority denied");
                    };
                    let Some(extension_id) = params.get("extensionId").and_then(Value::as_str)
                    else {
                        return LocalResponse::err("invalid grant request");
                    };
                    let Some(root) = params.get("root").and_then(Value::as_str) else {
                        return LocalResponse::err("invalid grant request");
                    };
                    let Some(provenance) = params
                        .get("provenance")
                        .and_then(Value::as_str)
                        .and_then(GrantProvenance::parse)
                    else {
                        return LocalResponse::err("invalid grant request");
                    };
                    let exact_file = params
                        .get("exactFile")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    match grants.register(
                        &grant_owner,
                        extension_id,
                        Path::new(root),
                        exact_file,
                        provenance,
                        None,
                    ) {
                        Ok((grant_id, _identity, persistent_id)) => {
                            LocalResponse::ok(serde_json::json!(
                                {"grantId": grant_id,
                                "persistentGrantId": persistent_id,
                                "exactFile": exact_file,
                                "provenance": provenance.as_str()}))
                        }
                        Err(_) => LocalResponse::err("grant authority denied"),
                    }
                }
                "grant.authority.reopen" => {
                    let (Some(persistent_id), Some(extension_id)) = (
                        params.get("persistentGrantId").and_then(Value::as_str),
                        params.get("extensionId").and_then(Value::as_str),
                    ) else {
                        return LocalResponse::err("invalid grant request");
                    };
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return LocalResponse::err("grant authority denied");
                    };
                    match grants.reopen(&grant_owner, persistent_id, extension_id) {
                        Ok((grant_id, _)) => {
                            LocalResponse::ok(serde_json::json!({"grantId": grant_id}))
                        }
                        Err(_) => LocalResponse::err("grant authority denied"),
                    }
                }
                "grant.snapshot.reserve" => {
                    if params.get("root").is_some()
                        || params.get("path").is_some()
                        || params.get("identityDev").is_some()
                        || params.get("identityIno").is_some()
                        || params.get("exactFile").is_some()
                    {
                        return LocalResponse::err("grant authority denied");
                    }
                    let grant_id = params.get("grantId").and_then(Value::as_str);
                    if grant_id.is_none() {
                        return LocalResponse::err("grant authority denied");
                    }
                    let (Some(grant_id), Some(extension_id), Some(relative)) = (
                        params.get("grantId").and_then(Value::as_str),
                        params.get("extensionId").and_then(Value::as_str),
                        params.get("relativeAsset").and_then(Value::as_str),
                    ) else {
                        return LocalResponse::err("invalid grant request");
                    };
                    let Some(grant_owner) =
                        desktop_authority
                            .owner(connection_id)
                            .map(|(session_id, generation)| GrantOwner {
                                session_id,
                                generation,
                                connection_id,
                            })
                    else {
                        return LocalResponse::err("grant authority denied");
                    };
                    let requested: Vec<&str> = relative
                        .split('/')
                        .filter(|part| !part.is_empty())
                        .collect();
                    if requested.is_empty()
                        || requested.iter().any(|part| *part == "." || *part == "..")
                    {
                        return LocalResponse::err("grant snapshot denied");
                    }
                    match grants.read(
                        grant_id,
                        &grant_owner,
                        extension_id,
                        &requested,
                        16 * 1024 * 1024,
                    ) {
                        Ok(bytes) => {
                            match snapshots.reserve(&owner, extension_id, "grant", bytes) {
                                Ok(handle) => {
                                    LocalResponse::ok(serde_json::json!({"handle": handle}))
                                }
                                Err(_) => LocalResponse::err("grant snapshot denied"),
                            }
                        }
                        Err(_) => LocalResponse::err("grant snapshot denied"),
                    }
                }
                "package.snapshot.chunk" | "grant.snapshot.chunk" => {
                    let handle = params.get("handle").and_then(Value::as_str);
                    let offset = params
                        .get("offset")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize);
                    let length = params
                        .get("length")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize);
                    match (handle, offset, length) {
                        (Some(handle), Some(offset), Some(length)) => {
                            match snapshots.chunk(handle, &owner, offset, length) {
                                Ok(bytes) => LocalResponse::ok(serde_json::json!({
                                    "handle": handle,
                                    "offset": offset,
                                    "bytes": base64::engine::general_purpose::STANDARD.encode(
                                        bytes
                                    ),
                                })),
                                Err(error) => LocalResponse::err(error.to_string()),
                            }
                        }
                        _ => LocalResponse::err("invalid package snapshot chunk"),
                    }
                }
                "package.snapshot.close" | "grant.snapshot.close" => {
                    match params.get("handle").and_then(Value::as_str) {
                        Some(handle) => match snapshots.close(handle, &owner) {
                            Ok(()) => LocalResponse::ok(serde_json::json!({ "closed": true })),
                            Err(error) => LocalResponse::err(error.to_string()),
                        },
                        None => LocalResponse::err("invalid package snapshot handle"),
                    }
                }
                _ => LocalResponse::err("unknown package snapshot operation"),
            }
        }
    };
    response
}
