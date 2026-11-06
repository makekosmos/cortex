use super::WorkerMethod;
use crate::package_manifest::PackageManifest;
use rand::RngCore;
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const READ_OPS: &[&str] = &[
    "list_objects",
    "get_object",
    "search_objects",
    "list_object_types",
    "get_object_type",
    "list_object_links",
    "get_sync_kv",
];
const WRITE_OPS: &[&str] = &[
    "upsert_object",
    "delete_object",
    "upsert_object_type",
    "delete_object_type",
    "upsert_object_link",
    "delete_object_link",
    "set_sync_kv",
    "external_refs.upsert",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub package_id: String,
    pub version: String,
    pub hash: String,
    pub pid: u32,
    pub generation: u64,
    pub correlation_id: String,
    token_hash: [u8; 32],
    pub scopes: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    InvalidManifest,
    UnsupportedCapability,
    InvalidScope,
}

impl Grant {
    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        manifest: &PackageManifest,
        hash: String,
        pid: u32,
        generation: u64,
        correlation_id: String,
        roots: &[PathBuf],
    ) -> Result<(Self, String), GrantError> {
        manifest
            .validate()
            .map_err(|_| GrantError::InvalidManifest)?;
        let mut scopes = HashMap::new();
        for p in &manifest.permissions {
            let normalized = match p.capability.as_str() {
                "ark.read" => validate_ops(&p.scopes, READ_OPS)?,
                "ark.write" => validate_ops(&p.scopes, WRITE_OPS)?,
                "network" => p
                    .scopes
                    .iter()
                    .map(|s| normalize_origin(s))
                    .collect::<Result<_, _>>()?,
                "filesystem.read" | "filesystem.write" | "process.spawn" => p
                    .scopes
                    .iter()
                    .map(|scope| normalize_path(scope, roots))
                    .collect::<Result<_, _>>()?,
                "worker.invoke"
                    if p.scopes
                        .iter()
                        .all(|scope| valid_worker_operation_scope(scope)) =>
                {
                    p.scopes.clone()
                }
                "dictation.control" => {
                    if p.scopes.iter().any(|scope| {
                        crate::runtime_grants::dictation_operation_capability(scope)
                            != Some("dictation.control")
                    }) {
                        return Err(GrantError::InvalidScope);
                    }
                    p.scopes.clone()
                }
                "worker.invoke" => return Err(GrantError::InvalidScope),
                "clipboard" | "notifications" => return Err(GrantError::UnsupportedCapability),
                _ => return Err(GrantError::UnsupportedCapability),
            };
            if normalized.is_empty() {
                return Err(GrantError::InvalidScope);
            }
            scopes.insert(p.capability.clone(), normalized);
        }
        let mut token = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut token);
        let token_string = hex_encode(&token);
        let token_hash = Sha256::digest(token_string.as_bytes());
        let mut stored = [0u8; 32];
        stored.copy_from_slice(&token_hash);
        Ok((
            Self {
                package_id: manifest.id.clone(),
                version: manifest.version.clone(),
                hash,
                pid,
                generation,
                correlation_id,
                token_hash: stored,
                scopes,
            },
            token_string,
        ))
    }

    pub fn authorize(
        &self,
        token: &str,
        pid: u32,
        generation: u64,
        api_major: u32,
        expected_api_major: u32,
        method: &WorkerMethod,
        scope: Option<&str>,
    ) -> bool {
        if !self.authenticate(token, pid, generation, api_major, expected_api_major) {
            return false;
        }
        let cap = match method {
            WorkerMethod::FilesystemRootOpen => "filesystem.read",
            WorkerMethod::ArkRead => "ark.read",
            WorkerMethod::ArkWrite => "ark.write",
            WorkerMethod::NetworkFetch => "network",
            WorkerMethod::FilesystemRead => "filesystem.read",
            WorkerMethod::FilesystemWrite
            | WorkerMethod::FilesystemDelete
            | WorkerMethod::FilesystemCreateDir => "filesystem.write",
            WorkerMethod::FilesystemList | WorkerMethod::FilesystemPoll => "filesystem.read",
            WorkerMethod::ProcessSpawn => "process.spawn",
        };
        let scopes = scope
            .filter(|operation| {
                crate::runtime_grants::dictation_operation_capability(operation)
                    == Some("dictation.control")
            })
            .and_then(|_| self.scopes.get("dictation.control"))
            .or_else(|| self.scopes.get(cap));
        match (scopes, scope) {
            (Some(xs), Some(s)) => xs.iter().any(|x| x == s),
            _ => false,
        }
    }

    pub fn authenticate(
        &self,
        token: &str,
        pid: u32,
        generation: u64,
        api_major: u32,
        expected_api_major: u32,
    ) -> bool {
        if pid != self.pid || generation != self.generation || api_major != expected_api_major {
            return false;
        }
        let digest = Sha256::digest(token.as_bytes());
        constant_time_eq(&self.token_hash, digest.as_slice())
    }
}

fn validate_ops(scopes: &[String], allowed: &[&str]) -> Result<Vec<String>, GrantError> {
    if scopes.is_empty()
        || scopes.iter().any(|s| !allowed.contains(&s.as_str()))
        || scopes.iter().collect::<HashSet<_>>().len() != scopes.len()
    {
        return Err(GrantError::InvalidScope);
    }
    Ok(scopes.to_vec())
}

fn valid_worker_operation_scope(scope: &str) -> bool {
    let operation = scope.strip_suffix(".*").unwrap_or(scope);
    !operation.is_empty()
        && operation.len() <= 128
        && operation.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

pub(super) fn normalize_origin(value: &str) -> Result<String, GrantError> {
    let u = Url::parse(value).map_err(|_| GrantError::InvalidScope)?;
    if (u.scheme() != "https"
        && !(cfg!(feature = "package-worker-fixture")
            && u.scheme() == "http"
            && u.host_str()
                .and_then(|host| host.parse::<std::net::IpAddr>().ok())
                .is_some_and(|ip| ip.is_loopback())))
        || u.username() != ""
        || u.password().is_some()
        || u.fragment().is_some()
        || u.path() != "/"
        || u.query().is_some()
    {
        return Err(GrantError::InvalidScope);
    }
    let host = u
        .host_str()
        .ok_or(GrantError::InvalidScope)?
        .to_ascii_lowercase();
    let local_test_origin = cfg!(feature = "package-worker-fixture")
        && u.scheme() == "http"
        && host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if host == "localhost"
        || host.ends_with(".localhost")
        || (!local_test_origin
            && host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .map(is_private)
                .unwrap_or(false))
    {
        return Err(GrantError::InvalidScope);
    }
    Ok(u.origin().ascii_serialization())
}

fn is_private(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => {
            v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_multicast()
                || v.is_broadcast()
                || v.is_unspecified()
        }
        std::net::IpAddr::V6(v) => {
            v.to_ipv4_mapped().is_some_and(|v4| is_private(v4.into()))
                || v.is_loopback()
                || v.is_unspecified()
                || v.is_unicast_link_local()
                || v.is_unique_local()
                || v.is_multicast()
        }
    }
}

pub(super) fn normalize_path(value: &str, roots: &[PathBuf]) -> Result<String, GrantError> {
    if value.starts_with("\\\\")
        || value.starts_with("//")
        || value.starts_with("\\\\?\\")
        || value.starts_with("\\\\.\\")
    {
        return Err(GrantError::InvalidScope);
    }
    let p = PathBuf::from(value.replace('\\', "/"));
    if !p.is_absolute() {
        return Err(GrantError::InvalidScope);
    }
    let mut clean = PathBuf::new();
    for component in p.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return Err(GrantError::InvalidScope),
            component => clean.push(component.as_os_str()),
        }
    }
    if roots.iter().any(|root| path_within(&clean, root)) {
        Ok(clean.to_string_lossy().into_owned())
    } else {
        Err(GrantError::InvalidScope)
    }
}

fn path_within(path: &std::path::Path, root: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        let path: Vec<_> = path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect();
        let root: Vec<_> = root
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect();
        path.len() >= root.len() && path[..root.len()] == root
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut x = 0u8;
    for (u, v) in a.iter().zip(b) {
        x |= u ^ v;
    }
    x == 0
}
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
