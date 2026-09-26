//! `filesystem.*` Engine operations for vault-style access (KOS-155).
//!
//! Vault roots are `GrantAuthorityRegistry` grants bound to the calling
//! process (`{class}:{pid}` owner), so a root opened in one `/v1/rpc` call can
//! be used by later calls from the same app instance — and never by another.
//! Responses carry relative paths only; absolute paths stay inside Engine.
//! Read/scan = `filesystem.read` scope; export = `filesystem.write` scope.

use super::*;

/// Per-process cap on open vault roots — `/v1/rpc` has no disconnect hook, so
/// this bounds retained root handles if an app forgets `vault.close`.
const MAX_VAULT_ROOTS_PER_OWNER: usize = 16;

fn vault_owner(client: &crate::engine_dispatch::DispatchClient) -> GrantOwner {
    let pid = client
        .pid
        .or_else(|| client.connection_id.and_then(|id| u32::try_from(id).ok()))
        .unwrap_or(0);
    GrantOwner {
        session_id: format!("vault:{}:{}", client.class.as_deref().unwrap_or("app"), pid),
        generation: 0,
        connection_id: u64::from(pid),
    }
}

fn vault_extension(client: &crate::engine_dispatch::DispatchClient) -> String {
    client
        .class
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "app".to_string())
}

fn grant_error(error: crate::grant_authority::GrantError) -> String {
    use crate::grant_authority::GrantError::*;
    match error {
        NotFound => "not-found".to_string(),
        OwnerMismatch | ExtensionMismatch | ScopeMismatch | IdentityChanged => {
            "forbidden".to_string()
        }
        Invalid => "invalid-request".to_string(),
        Persistence => "unavailable".to_string(),
    }
}

pub(super) async fn open_vault(
    params: serde_json::Value,
    client: &crate::engine_dispatch::DispatchClient,
    grants: &Arc<GrantAuthorityRegistry>,
    scan: bool,
) -> LocalResponse {
    let Some(path) = params.get("path").and_then(Value::as_str) else {
        return LocalResponse::err("invalid-request");
    };
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        return LocalResponse::err("invalid-request");
    }
    let owner = vault_owner(client);
    if grants.owner_grant_count(&owner) >= MAX_VAULT_ROOTS_PER_OWNER {
        return LocalResponse::err("unavailable");
    }
    let extension = vault_extension(client);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let grants = grants.clone();
    let scanned = scan;
    let result = tokio::task::spawn_blocking(move || {
        let (grant_id, _identity, _persistent) = grants.register(
            &owner,
            &extension,
            &path,
            false,
            GrantProvenance::PersistedUserData,
            None,
        )?;
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone())
            .to_string_lossy()
            .to_string();
        let vault_key = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(canonical.as_bytes());
            digest[..16]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        if !scanned {
            return Ok((
                grant_id,
                vault_key,
                crate::markdown_vault::VaultScan::default(),
            ));
        }
        match grants.with_directory_root(&grant_id, &owner, &extension, |root| {
            crate::markdown_vault::scan_root(root, &path)
        }) {
            Ok(scan) => Ok((grant_id, vault_key, scan)),
            Err(error) => {
                grants.close(&grant_id, &owner);
                Err(error)
            }
        }
    })
    .await;
    match result {
        Ok(Ok((root_id, vault_key, scan))) => {
            let mut data =
                serde_json::json!({ "rootId": root_id, "name": name, "vaultKey": vault_key });
            if scanned {
                data["files"] = serde_json::to_value(&scan.files).unwrap_or_default();
                data["images"] = serde_json::to_value(&scan.images).unwrap_or_default();
            }
            LocalResponse::ok(data)
        }
        Ok(Err(error)) => LocalResponse::err(grant_error(error)),
        Err(_) => LocalResponse::err("unavailable"),
    }
}

fn grant_components(params: &serde_json::Value) -> Result<(&str, Vec<String>), &'static str> {
    let root_id = params
        .get("rootId")
        .and_then(Value::as_str)
        .ok_or("invalid-request")?;
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or("invalid-request")?;
    let components =
        crate::markdown_vault::parse_relative_path(path).map_err(|_| "invalid-request")?;
    Ok((root_id, components))
}

pub(super) async fn read_vault_file(
    params: serde_json::Value,
    client: &crate::engine_dispatch::DispatchClient,
    grants: &Arc<GrantAuthorityRegistry>,
) -> LocalResponse {
    let (root_id, components) = match grant_components(&params) {
        Ok(value) => value,
        Err(error) => return LocalResponse::err(error),
    };
    let owner = vault_owner(client);
    let extension = vault_extension(client);
    let grants = grants.clone();
    let root_id = root_id.to_string();
    let result = tokio::task::spawn_blocking(move || {
        grants.with_directory_root(&root_id, &owner, &extension, |root| {
            let borrowed: Vec<&str> = components.iter().map(String::as_str).collect();
            crate::handle_relative_fs::read_relative(
                root,
                &borrowed,
                crate::markdown_vault::MARKDOWN_IMAGE_MAX_BYTES as usize,
            )
        })
    })
    .await;
    match result {
        Ok(Ok(bytes)) => LocalResponse::ok(serde_json::json!({
            "bytesBase64": base64::engine::general_purpose::STANDARD.encode(bytes),
        })),
        Ok(Err(error)) => LocalResponse::err(grant_error(error)),
        Err(_) => LocalResponse::err("unavailable"),
    }
}

pub(super) async fn export_vault(
    params: serde_json::Value,
    client: &crate::engine_dispatch::DispatchClient,
    grants: &Arc<GrantAuthorityRegistry>,
) -> LocalResponse {
    let Some(root_id) = params.get("rootId").and_then(Value::as_str) else {
        return LocalResponse::err("invalid-request");
    };
    let Some(files) = params.get("files").and_then(Value::as_array) else {
        return LocalResponse::err("invalid-request");
    };
    let mut plan = Vec::with_capacity(files.len());
    for file in files {
        let Some(relative) = file.get("relativePath").and_then(Value::as_str) else {
            return LocalResponse::err("invalid-request");
        };
        let components = match crate::markdown_vault::parse_relative_path(relative) {
            Ok(components) => components,
            Err(_) => return LocalResponse::err("invalid-request"),
        };
        let content = file.get("content").and_then(Value::as_str);
        let source = file.get("sourcePath").and_then(Value::as_str);
        match (content, source) {
            (Some(content), None) => plan.push(crate::markdown_vault::VaultExportFile::Text {
                components,
                content: content.to_string(),
            }),
            (None, Some(source)) => {
                // Unresolvable sources are skipped like a failed asset copy;
                // the entry still occupies a plan slot for the count caps.
                if let Some(source_path) = crate::markdown_vault::resolve_vault_source_path(source)
                {
                    plan.push(crate::markdown_vault::VaultExportFile::Copy {
                        components,
                        source: source_path,
                    });
                }
            }
            _ => return LocalResponse::err("invalid-request"),
        }
    }
    let owner = vault_owner(client);
    let extension = vault_extension(client);
    let grants = grants.clone();
    let root_id = root_id.to_string();
    let result = tokio::task::spawn_blocking(move || {
        grants.with_directory_root(&root_id, &owner, &extension, |root| {
            crate::markdown_vault::export_files(root, &plan)
        })
    })
    .await;
    match result {
        Ok(Ok(exported)) => LocalResponse::ok(serde_json::json!({ "exportedCount": exported })),
        Ok(Err(error)) => LocalResponse::err(grant_error(error)),
        Err(_) => LocalResponse::err("unavailable"),
    }
}

pub(in crate::ws_server) async fn handle_filesystem_op(
    subop: &str,
    params: serde_json::Value,
    client: &crate::engine_dispatch::DispatchClient,
    grants: &Arc<GrantAuthorityRegistry>,
) -> LocalResponse {
    match subop {
        "vault.open" => open_vault(params, client, grants, true).await,
        "vault.register" => open_vault(params, client, grants, false).await,
        "vault.read" => read_vault_file(params, client, grants).await,
        "vault.export" => export_vault(params, client, grants).await,
        "vault.close" => {
            let Some(root_id) = params.get("rootId").and_then(Value::as_str) else {
                return LocalResponse::err("invalid-request");
            };
            let closed = grants.close(root_id, &vault_owner(client));
            LocalResponse::ok(serde_json::json!({ "closed": closed }))
        }
        _ => LocalResponse::err("unknown-operation"),
    }
}
