//! Worker JSON-lines protocol and in-memory capability grants.

use crate::package_manifest::{IntegrationSchedule, IntegrationSetting, PackageManifest};
use rand::RngCore;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// Stable worker-facing protocol types live in the standalone
// `package-protocol` crate so out-of-tree workers can pin them by git
// rev without depending on the runtime source tree.
pub use package_protocol::{BridgeStatus, BridgeWorkerConfig};

pub const MAX_LINE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum WorkerMessage {
    Bootstrap(BootstrapMessage),
    Hello(HelloMessage),
    Heartbeat(HeartbeatMessage),
    Call(CallMessage),
    Invoke(InvokeMessage),
    Stop(StopMessage),
    Result(ResultMessage),
    Event(EventMessage),
    Error(ErrorMessage),
}

macro_rules! msg {
    ($name:ident { $($body:tt)* }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($body)*
        }
    };
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationBootstrapConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site_id: Option<u32>,
    pub settings: Vec<IntegrationSetting>,
    pub values: HashMap<String, String>,
    pub secret_handles: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<IntegrationSchedule>,
}
impl IntegrationBootstrapConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self
            .account_key
            .as_ref()
            .is_some_and(|key| key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid-account-key");
        }
        if self.settings.len() > 64
            || self.values.len() > self.settings.len()
            || self.secret_handles.len() > self.settings.len()
        {
            return Err("invalid-request");
        }
        let mut keys = HashSet::new();
        for setting in &self.settings {
            if setting.key.is_empty()
                || setting.key.len() > 64
                || !setting
                    .key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                || setting.label.is_empty()
                || setting.label.len() > 128
                || setting.label.chars().any(char::is_control)
                || setting.description.as_ref().is_some_and(|description| {
                    description.len() > 512 || description.chars().any(char::is_control)
                })
                || !keys.insert(&setting.key)
            {
                return Err("invalid-request");
            }
        }
        for (key, value) in &self.values {
            if !keys.contains(key)
                || self
                    .settings
                    .iter()
                    .any(|setting| setting.key == *key && setting.kind.is_secret())
                || value.is_empty()
                || value.len() > 4096
                || value.trim() != value
                || value.chars().any(char::is_control)
            {
                return Err("invalid-request");
            }
        }
        for (key, token) in &self.secret_handles {
            if !keys.contains(key)
                || self
                    .settings
                    .iter()
                    .any(|setting| setting.key == *key && !setting.kind.is_secret())
                || token.len() != 64
                || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("invalid-request");
            }
        }
        for setting in &self.settings {
            if !setting.required {
                continue;
            }
            let present = if setting.kind.is_secret() {
                self.secret_handles.contains_key(&setting.key)
            } else {
                self.values.contains_key(&setting.key)
            };
            if !present {
                return Err("invalid-request");
            }
        }
        if self
            .schedule
            .as_ref()
            .is_some_and(|schedule| !(60..=7 * 24 * 60 * 60).contains(&schedule.interval_seconds))
        {
            return Err("invalid-request");
        }
        Ok(())
    }
}
msg!(BootstrapMessage {
    pub method: String,
    pub package_id: String,
    pub version: String,
    pub hash: String,
    pub pid: u32,
    pub api_version: u32,
    pub generation: u64,
    pub correlation_id: String,
    pub token: String,
    #[serde(default)]
    pub bridge_config: Option<BridgeWorkerConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration: Option<IntegrationBootstrapConfig>,
});
msg!(RunMessage { pub method: String, pub generation: u64, pub run_id: String });
impl RunMessage {
    fn validate(&self) -> Result<(), &'static str> {
        (!self.run_id.is_empty()
            && self.run_id.len() <= 128
            && !self.run_id.chars().any(char::is_control))
        .then_some(())
        .ok_or("invalid-request")
    }
}
msg!(HelloMessage {
    pub method: String,
    pub package_id: String,
    pub version: String,
    pub hash: String,
    pub pid: u32,
    pub api_version: u32,
    pub token: String,
});
msg!(HeartbeatMessage {
    pub method: String,
    pub generation: u64,
    pub token: String,
    #[serde(default)] pub bridge_status: Option<BridgeStatus>,
});
msg!(CallMessage {
    pub method: String,
    pub id: String,
    pub generation: u64,
    pub token: String,
    pub operation: WorkerMethod,
    pub params: serde_json::Value,
});
msg!(InvokeMessage {
    pub method: String,
    pub id: String,
    pub generation: u64,
    pub operation: String,
    pub params: serde_json::Value,
});
msg!(StopMessage { pub method: String, pub generation: u64, pub reason: String });
msg!(ResultMessage {
    pub method: String,
    pub id: String,
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
});
msg!(EventMessage {
    pub method: String,
    pub event: String,
    #[serde(default)] pub data: Option<serde_json::Value>,
});
msg!(ErrorMessage { pub method: String, pub error: String });

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WorkerMethod {
    #[serde(rename = "filesystem.root.open")]
    FilesystemRootOpen,
    #[serde(rename = "ark.read")]
    ArkRead,
    #[serde(rename = "ark.write")]
    ArkWrite,
    #[serde(rename = "network.fetch")]
    NetworkFetch,
    #[serde(rename = "filesystem.read")]
    FilesystemRead,
    #[serde(rename = "filesystem.write")]
    FilesystemWrite,
    #[serde(rename = "filesystem.list")]
    FilesystemList,
    #[serde(rename = "filesystem.poll")]
    FilesystemPoll,
    #[serde(rename = "filesystem.delete")]
    FilesystemDelete,
    #[serde(rename = "filesystem.mkdir")]
    FilesystemCreateDir,
    #[serde(rename = "process.spawn")]
    ProcessSpawn,
}

pub fn parse_json_line(line: &[u8]) -> Result<WorkerMessage, &'static str> {
    if line.len() > MAX_LINE_BYTES {
        return Err("invalid-request");
    }
    let message: WorkerMessage = serde_json::from_slice(line).map_err(|_| "invalid-request")?;
    let valid_method = match &message {
        WorkerMessage::Bootstrap(message) => {
            message.method == "worker.bootstrap"
                && message
                    .integration
                    .as_ref()
                    .is_none_or(|config| config.validate().is_ok())
        }
        WorkerMessage::Hello(message) => message.method == "worker.hello",
        WorkerMessage::Heartbeat(message) => message.method == "worker.heartbeat",
        WorkerMessage::Call(message) => message.method == "worker.call",
        WorkerMessage::Invoke(message) => {
            message.method == "worker.invoke"
                && !message.id.is_empty()
                && message.id.len() <= 128
                && !message.operation.is_empty()
                && message.operation.len() <= 128
                && message.operation.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        }
        WorkerMessage::Stop(message) => message.method == "worker.stop",
        WorkerMessage::Result(message) => message.method == "worker.result",
        WorkerMessage::Event(message) => {
            message.method == "worker.event"
                && !message.event.is_empty()
                && message.event.len() <= 128
                && !message.event.chars().any(char::is_control)
        }
        WorkerMessage::Error(message) => {
            message.method == "worker.error"
                && !message.error.is_empty()
                && message.error.len() <= 256
                && !message.error.chars().any(char::is_control)
        }
    };
    valid_method.then_some(message).ok_or("invalid-request")
}

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

fn normalize_origin(value: &str) -> Result<String, GrantError> {
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

fn normalize_path(value: &str, roots: &[PathBuf]) -> Result<String, GrantError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package_manifest::{PackageKind, PermissionRequest};

    fn manifest(capability: &str, scopes: Vec<&str>) -> PackageManifest {
        PackageManifest {
            schema_version: 1,
            id: "pkg".into(),
            name: "Pkg".into(),
            version: "1.0.0".into(),
            kind: PackageKind::Source,
            engine_api: ">=1".into(),
            entrypoint: "worker.exe".into(),
            publisher: "kosmos".into(),
            permissions: vec![PermissionRequest {
                capability: capability.into(),
                scopes: scopes.into_iter().map(str::to_owned).collect(),
            }],
        }
    }

    #[test]
    fn token_pid_generation_and_allowlist_are_bound() {
        let (grant, token) = Grant::derive(
            &manifest("ark.read", vec!["get_object"]),
            "h".into(),
            7,
            2,
            "c".into(),
            &[],
        )
        .unwrap();
        assert!(grant.authorize(
            &token,
            7,
            2,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("get_object")
        ));
        assert!(!grant.authorize(
            "wrong",
            7,
            2,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("get_object")
        ));
        assert!(!grant.authorize(
            &token,
            8,
            2,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("get_object")
        ));
        assert!(!grant.authorize(
            &token,
            7,
            3,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("get_object")
        ));
        assert!(!grant.authorize(&token, 7, 2, 1, 1, &WorkerMethod::ArkRead, None));
        assert!(Grant::derive(
            &manifest("clipboard", vec![]),
            "h".into(),
            1,
            1,
            "c".into(),
            &[]
        )
        .is_err());
    }

    #[test]
    fn dictation_control_authorizes_only_registered_engine_operations() {
        let (grant, token) = Grant::derive(
            &manifest("dictation.control", vec!["dictation.capture.start"]),
            "h".into(),
            7,
            2,
            "c".into(),
            &[],
        )
        .unwrap();
        assert!(grant.authorize(
            &token,
            7,
            2,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("dictation.capture.start")
        ));
        assert!(!grant.authorize(
            &token,
            7,
            2,
            1,
            1,
            &WorkerMethod::ArkRead,
            Some("dictation.submit_audio")
        ));
        assert!(Grant::derive(
            &manifest("dictation.control", vec!["dictation.submit_audio"]),
            "h".into(),
            1,
            1,
            "c".into(),
            &[]
        )
        .is_err());
    }

    #[test]
    fn network_and_path_scopes_fail_closed() {
        assert!(normalize_origin("http://example.com/").is_err());
        assert!(normalize_origin("https://127.0.0.1/").is_err());
        assert!(normalize_origin("https://[::ffff:127.0.0.1]/").is_err());
        assert!(normalize_origin("https://[fc00::1]/").is_err());
        assert!(normalize_origin("https://user@example.com/").is_err());
        let root = std::env::temp_dir();
        assert!(normalize_path("relative", std::slice::from_ref(&root)).is_err());
        assert!(normalize_path("//server/share", std::slice::from_ref(&root)).is_err());
        assert!(normalize_path(
            &root.join("child").to_string_lossy(),
            std::slice::from_ref(&root)
        )
        .is_ok());
        assert!(normalize_path(
            &root.join("child/../escape").to_string_lossy(),
            std::slice::from_ref(&root)
        )
        .is_err());
    }

    #[test]
    fn empty_path_capabilities_fail_closed() {
        let root = std::env::temp_dir();
        for capability in ["filesystem.read", "filesystem.write", "process.spawn"] {
            assert!(matches!(
                Grant::derive(
                    &manifest(capability, vec![]),
                    "h".into(),
                    1,
                    1,
                    "c".into(),
                    std::slice::from_ref(&root),
                ),
                Err(GrantError::InvalidScope)
            ));
        }
    }

    #[test]
    fn malformed_and_oversized_lines_rejected() {
        assert!(parse_json_line(b"{}").is_err());
        assert!(matches!(
            parse_json_line(
                "{\"method\":\"worker.call\",\"id\":\"1\",\"generation\":1,\"token\":\"x\",\
                    \"operation\":\"ark.read\",\"params\":{}}"
                    .as_bytes()
            ),
            Ok(WorkerMessage::Call(_))
        ));
        assert!(parse_json_line(
            "{\"method\":\"ark.read\",\"id\":\"1\",\"generation\":1,\"token\":\"x\",\
                \"operation\":\"ark.read\",\"params\":{}}"
                .as_bytes()
        )
        .is_err());
        assert!(matches!(
            parse_json_line(
                br#"{"method":"worker.event","event":"dictation.error","data":{"retryable":true}}"#
            ),
            Ok(WorkerMessage::Event(_))
        ));
        assert!(matches!(
            parse_json_line(br#"{"method":"worker.error","error":"unknown-operation"}"#),
            Ok(WorkerMessage::Error(_))
        ));
        assert!(parse_json_line(&vec![b'a'; MAX_LINE_BYTES + 1]).is_err());
    }

    #[test]
    fn integration_bootstrap_config_is_metadata_and_handle_only() {
        let value = serde_json::json!({
            "settings": [
                {"key": "username", "label": "Username", "kind": "text"},
                {"key": "api_key", "label": "API key", "kind": "secret"}
            ],
            "values": {"username": "bigfrontend-user"},
            "secret_handles": {
                "api_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            },
            "schedule": {"interval_seconds": 3600}
        });
        let mut config: IntegrationBootstrapConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.account_key, None);
        config.settings[0].required = true;
        config.settings[1].required = true;
        assert!(config.validate().is_ok());
        let wire = serde_json::to_string(&config).unwrap();
        assert!(wire.contains("bigfrontend-user"));
        assert!(wire.contains("0123456789abcdef"));
        assert!(!wire.contains("secret-value"));
        assert!(serde_json::from_str::<IntegrationBootstrapConfig>(
            &wire.replace("secret_handles", "unexpected")
        )
        .is_err());

        let invalid = |values, secret_handles| IntegrationBootstrapConfig {
            account_key: None,
            data_origin: None,
            site_id: None,
            settings: config.settings.clone(),
            values,
            secret_handles,
            schedule: None,
        };
        assert!(invalid(
            [("api_key".into(), "secret-value".into())].into(),
            HashMap::new()
        )
        .validate()
        .is_err());
        assert!(invalid(
            HashMap::new(),
            [(
                "username".into(),
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into()
            )]
            .into()
        )
        .validate()
        .is_err());
        assert!(IntegrationBootstrapConfig {
            account_key: None,
            data_origin: None,
            site_id: None,
            settings: config.settings.clone(),
            values: HashMap::new(),
            secret_handles: HashMap::new(),
            schedule: None,
        }
        .validate()
        .is_err());
        assert!(IntegrationBootstrapConfig {
            account_key: None,
            data_origin: None,
            site_id: None,
            settings: config.settings.clone(),
            values: [("username".into(), "user".into())].into(),
            secret_handles: HashMap::new(),
            schedule: None,
        }
        .validate()
        .is_err());
        assert!(
            invalid([("other".into(), "value".into())].into(), HashMap::new())
                .validate()
                .is_err()
        );
        assert!(IntegrationBootstrapConfig {
            account_key: Some("not-a-sha256-digest".into()),
            data_origin: None,
            site_id: None,
            settings: config.settings,
            values: HashMap::new(),
            secret_handles: HashMap::new(),
            schedule: None,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn run_message_is_strict_and_method_bound() {
        let run: RunMessage =
            serde_json::from_slice(br#"{"method":"worker.run","generation":7,"run_id":"run-1"}"#)
                .unwrap();
        assert!(run.validate().is_ok());
        assert!(serde_json::from_slice::<RunMessage>(
            br#"{"method":"worker.run","generation":7,"run_id":"run-1","extra":true}"#
        )
        .is_err());
        assert!(
            parse_json_line(br#"{"method":"worker.run","generation":7,"run_id":"run-1"}"#).is_err()
        );
    }

    #[test]
    fn opaque_root_open_is_a_strict_worker_call() {
        let message = parse_json_line(
            concat!(
                r#"{"method":"worker.call","id":"1","generation":7,"token":"token","#,
                r#""operation":"filesystem.root.open","params":{"persistent_grant_id":"#,
                r#""00000000-0000-4000-8000-000000000001"}}"#
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(matches!(
            message,
            WorkerMessage::Call(CallMessage {
                operation: WorkerMethod::FilesystemRootOpen,
                ..
            })
        ));
    }
}
