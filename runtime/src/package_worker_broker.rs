//! Brokered Engine API operations for package workers.
//!
//! This brokers the Engine API; it is not hostile native-code containment.

use crate::package_manifest::{IntegrationRequestMethod, SecretInjection};

#[cfg(windows)]
use std::io::Read;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use thiserror::Error;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_JSON_BODY: usize = 64 * 1024;
pub(crate) const MAX_NETWORK_RESPONSE: usize = 32 * 1024 * 1024;
pub const MAX_DIRECTORY_ENTRIES: usize = 4096;
const MAX_TEMPFILE_ATTEMPTS: usize = 128;
static TEMPFILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub modified_ms: u128,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("invalid request: {0}")]
    Invalid(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Clone, Debug)]
pub struct BrokerConfig {
    pub allowed_origins: HashSet<String>,
    pub filesystem_roots: Vec<PathBuf>,
    pub private_state_roots: Vec<PathBuf>,
    #[cfg(feature = "package-worker-fixture")]
    pub(crate) allow_local_test_origin: bool,
}

impl BrokerConfig {
    pub fn new<I, S>(origins: I, roots: Vec<PathBuf>) -> Result<Self, BrokerError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut allowed_origins = HashSet::new();
        for origin in origins {
            let url = reqwest::Url::parse(origin.as_ref())
                .map_err(|e| BrokerError::Invalid(e.to_string()))?;
            if (url.scheme() != "https"
                && !(cfg!(feature = "package-worker-fixture")
                    && url.scheme() == "http"
                    && url.host_str().is_some_and(is_loopback_host)))
                || url.username() != ""
                || url.password().is_some()
                || !(url.path().is_empty() || url.path() == "/")
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(BrokerError::Invalid("origins must be HTTPS origins".into()));
            }
            allowed_origins.insert(url.origin().ascii_serialization());
        }
        let filesystem_roots = roots
            .into_iter()
            .map(|p| std::fs::canonicalize(p))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            allowed_origins,
            filesystem_roots,
            private_state_roots: Vec::new(),
            #[cfg(feature = "package-worker-fixture")]
            allow_local_test_origin: false,
        })
    }

    #[cfg(feature = "package-worker-fixture")]
    pub(crate) fn enable_local_test_origin(mut self) -> Self {
        self.allow_local_test_origin = true;
        self
    }

    pub fn with_private_state_root(mut self, root: &Path) -> Result<Self, BrokerError> {
        self.private_state_roots.push(std::fs::canonicalize(root)?);
        Ok(self)
    }
}

pub struct SecretRequest<'a> {
    pub injection: &'a SecretInjection,
    pub secret: &'a str,
    pub allowed_cookie_names: &'a [String],
}

pub fn spawn_process(
    config: &BrokerConfig,
    executable: &Path,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<u32, BrokerError> {
    if args.len() > 64
        || args
            .iter()
            .any(|arg| arg.len() > 4096 || arg.chars().any(char::is_control))
    {
        return Err(BrokerError::Invalid("invalid process arguments".into()));
    }
    reject_path(executable)?;
    let executable = std::fs::canonicalize(executable)?;
    if !is_under_configured_root(config, &executable) {
        return Err(BrokerError::Invalid(
            "executable escapes configured roots".into(),
        ));
    }
    if !executable.is_file() {
        return Err(BrokerError::Invalid("executable is not a file".into()));
    }
    let cwd = cwd
        .map(|path| {
            reject_path(path)?;
            let path = std::fs::canonicalize(path)?;
            is_under_configured_root(config, &path)
                .then_some(path)
                .ok_or_else(|| {
                    BrokerError::Invalid("working directory escapes configured roots".into())
                })
        })
        .transpose()?;
    if cwd.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err(BrokerError::Invalid(
            "working directory is not a directory".into(),
        ));
    }
    let mut command = std::process::Command::new(executable);
    command
        .env_clear()
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for key in [
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "PATH",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    // ponytail: user-launched apps are detached and may outlive the package worker;
    // add an explicit managed-process mode before using this broker for helpers.
    Ok(command.spawn()?.id())
}

pub fn create_directory(config: &BrokerConfig, path: &Path) -> Result<(), BrokerError> {
    reject_path(path)?;
    let root = config
        .filesystem_roots
        .iter()
        .find(|root| path_is_under(root, path))
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    let relative = relative_components(root, path)
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    let mut current = root.clone();
    for component in relative {
        let next = current.join(component);
        match fs::create_dir(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        current = fs::canonicalize(next)?;
        if !current.is_dir() || !path_is_under(root, &current) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
    }
    Ok(())
}

fn relative_components(root: &Path, path: &Path) -> Option<Vec<std::ffi::OsString>> {
    let normalize = |component: std::path::Component<'_>| {
        component
            .as_os_str()
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_ascii_lowercase()
    };
    let root_components = root.components().map(normalize).collect::<Vec<_>>();
    let path_components = path.components().collect::<Vec<_>>();
    if path_components.len() < root_components.len()
        || path_components
            .iter()
            .take(root_components.len())
            .copied()
            .map(normalize)
            .ne(root_components.iter().cloned())
    {
        return None;
    }
    path_components[root_components.len()..]
        .iter()
        .map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect()
}

pub async fn fetch(config: &BrokerConfig, raw_url: &str) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret(config, raw_url, None).await
}

pub async fn fetch_with_secret(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret_json(config, raw_url, secret, None).await
}

pub async fn fetch_with_secret_json(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
    body: Option<&serde_json::Value>,
) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret_json_limit(config, raw_url, secret, body, MAX_BYTES).await
}

pub(crate) async fn fetch_with_secret_json_limit(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
    body: Option<&serde_json::Value>,
    limit: usize,
) -> Result<Vec<u8>, BrokerError> {
    if limit == 0 || limit > MAX_NETWORK_RESPONSE {
        return Err(BrokerError::Invalid("invalid response limit".into()));
    }
    let method = secret
        .as_ref()
        .map(|request| request.injection.request_method())
        .unwrap_or_default();
    let injected_body = inject_json_body(body, secret.as_ref())?;
    let body = request_body(method, injected_body.as_ref().or(body))?;
    let mut url = validate_url(config, raw_url)?;
    for redirect_count in 0..=3 {
        let host = url
            .host_str()
            .ok_or_else(|| BrokerError::Invalid("missing host".into()))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| BrokerError::Invalid("missing port".into()))?;
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await?.collect();
        let addr = addrs
            .into_iter()
            .find(|a| {
                #[cfg(feature = "package-worker-fixture")]
                if config.allow_local_test_origin && a.ip().is_loopback() {
                    return true;
                }
                !is_blocked_ip(a.ip())
            })
            .ok_or_else(|| BrokerError::Invalid("host resolves to blocked address".into()))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .resolve(host, addr)
            .build()?;
        let mut request_url = url.clone();
        if let Some(SecretRequest {
            injection: SecretInjection::Query { parameter, .. },
            secret,
            ..
        }) = secret.as_ref()
        {
            request_url.query_pairs_mut().append_pair(parameter, secret);
        }
        let mut request = match method {
            IntegrationRequestMethod::Get => client.get(request_url.clone()),
            IntegrationRequestMethod::PostJson => client
                .post(request_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone().unwrap_or_default()),
        };
        if let Some(secret) = secret.as_ref() {
            if !secret.injection.origins().iter().any(|origin| {
                reqwest::Url::parse(origin).is_ok_and(|allowed| allowed.origin() == url.origin())
            }) {
                return Err(BrokerError::Invalid("secret origin denied".into()));
            }
            request = apply_secret(request, secret)?;
        }
        let response = request.send().await?;
        if response.status().is_redirection() {
            if redirect_count == 3 {
                return Err(BrokerError::Invalid("too many redirects".into()));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| BrokerError::Invalid("redirect missing location".into()))?;
            url = validate_url(
                config,
                url.join(location)
                    .map_err(|e| BrokerError::Invalid(e.to_string()))?
                    .as_str(),
            )?;
            continue;
        }
        if !response.status().is_success() {
            return Err(BrokerError::Invalid(format!(
                "HTTP status {}",
                response.status()
            )));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(BrokerError::Invalid("response exceeds limit".into()));
        }
        let mut out = Vec::new();
        let mut stream = response;
        while let Some(chunk) = stream.chunk().await? {
            if out.len() + chunk.len() > limit {
                return Err(BrokerError::Invalid("response exceeds limit".into()));
            }
            out.extend_from_slice(&chunk);
        }
        return Ok(out);
    }
    unreachable!()
}

fn request_body(
    method: IntegrationRequestMethod,
    body: Option<&serde_json::Value>,
) -> Result<Option<Vec<u8>>, BrokerError> {
    Ok(match (method, body) {
        (IntegrationRequestMethod::Get, None) => None,
        (IntegrationRequestMethod::PostJson, Some(body)) => {
            let bytes = serde_json::to_vec(body)
                .map_err(|_| BrokerError::Invalid("invalid JSON body".into()))?;
            if bytes.len() > MAX_JSON_BODY {
                return Err(BrokerError::Invalid("JSON body exceeds 64 KiB".into()));
            }
            Some(bytes)
        }
        _ => return Err(BrokerError::Invalid("request method/body mismatch".into())),
    })
}

fn session_field<'a>(session: &'a serde_json::Value, field: &str) -> Result<&'a str, BrokerError> {
    session
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 8192)
        .ok_or_else(|| BrokerError::Invalid("missing or invalid session field".into()))
}

fn inject_json_body(
    body: Option<&serde_json::Value>,
    secret: Option<&SecretRequest<'_>>,
) -> Result<Option<serde_json::Value>, BrokerError> {
    let Some(SecretRequest {
        injection: SecretInjection::Json { body_fields, .. },
        secret,
        ..
    }) = secret
    else {
        return Ok(None);
    };
    let session: serde_json::Value = serde_json::from_str(secret)
        .map_err(|_| BrokerError::Invalid("invalid JSON session".into()))?;
    let mut body = body
        .and_then(serde_json::Value::as_object)
        .cloned()
        .ok_or_else(|| BrokerError::Invalid("JSON session requires an object body".into()))?;
    for (name, field) in body_fields {
        if body.contains_key(name) {
            return Err(BrokerError::Invalid(
                "body contains reserved session field".into(),
            ));
        }
        body.insert(
            name.clone(),
            serde_json::Value::String(session_field(&session, field)?.to_owned()),
        );
    }
    Ok(Some(serde_json::Value::Object(body)))
}

fn apply_secret(
    request: reqwest::RequestBuilder,
    secret: &SecretRequest<'_>,
) -> Result<reqwest::RequestBuilder, BrokerError> {
    match secret.injection {
        SecretInjection::Header { name, prefix, .. } => Ok(request.header(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| BrokerError::Invalid("invalid secret header".into()))?,
            reqwest::header::HeaderValue::from_str(&format!("{prefix}{}", secret.secret))
                .map_err(|_| BrokerError::Invalid("invalid secret header".into()))?,
        )),
        SecretInjection::Basic { password, .. } => {
            Ok(request.basic_auth(secret.secret, Some(password)))
        }
        SecretInjection::Query { .. } => Ok(request),
        SecretInjection::Json {
            header_fields,
            headers,
            ..
        } => {
            let session: serde_json::Value = serde_json::from_str(secret.secret)
                .map_err(|_| BrokerError::Invalid("invalid JSON session".into()))?;
            let mut request = request;
            for (name, value) in headers {
                request = request.header(name, value);
            }
            for (name, field) in header_fields {
                let value = reqwest::header::HeaderValue::from_str(session_field(&session, field)?)
                    .map_err(|_| BrokerError::Invalid("invalid session header".into()))?;
                request = request.header(name, value);
            }
            Ok(request)
        }
        SecretInjection::Cookies { .. } => {
            let values: HashMap<String, String> = serde_json::from_str(secret.secret)
                .map_err(|_| BrokerError::Invalid("invalid secret cookies".into()))?;
            if values.is_empty()
                || secret
                    .allowed_cookie_names
                    .iter()
                    .any(|name| !values.contains_key(name))
                || values.keys().any(|name| {
                    !secret
                        .allowed_cookie_names
                        .iter()
                        .any(|allowed| allowed == name)
                })
                || values.values().any(|value| {
                    value.is_empty()
                        || value.len() > 4096
                        || value
                            .chars()
                            .any(|character| matches!(character, ';' | '\r' | '\n'))
                })
            {
                return Err(BrokerError::Invalid("invalid secret cookies".into()));
            }
            let mut request = request;
            if let Some(headers) = secret.injection.fixed_headers() {
                for (name, value) in headers {
                    request = request.header(name, value);
                }
            }
            if let Some(mirror) = secret.injection.cookie_header() {
                request = request.header(
                    &mirror.header,
                    values
                        .get(&mirror.cookie)
                        .ok_or_else(|| BrokerError::Invalid("missing mirrored cookie".into()))?,
                );
            }
            let cookies = values
                .into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            Ok(request.header(reqwest::header::COOKIE, cookies))
        }
    }
}

pub const MAX_SNAPSHOT_CHUNK: usize = 256 * 1024;

struct SnapshotEntry {
    owner: String,
    bytes: Vec<u8>,
}

pub struct SnapshotRegistry {
    entries: Mutex<HashMap<String, SnapshotEntry>>,
    network: bool,
}

impl SnapshotRegistry {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            network: false,
        }
    }

    pub(crate) fn network_responses() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            network: true,
        }
    }

    pub fn reserve(
        &self,
        owner: &str,
        _package_id: &str,
        _source: &str,
        bytes: Vec<u8>,
    ) -> Result<String, BrokerError> {
        let limit = if self.network {
            MAX_NETWORK_RESPONSE
        } else {
            MAX_BYTES * 16
        };
        if bytes.len() > limit {
            return Err(BrokerError::Invalid("snapshot is too large".into()));
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.network
            && (entries.values().any(|entry| entry.owner == owner) || entries.len() >= 4)
        {
            return Err(BrokerError::Invalid(
                "network response capacity exhausted".into(),
            ));
        }
        let handle = uuid::Uuid::new_v4().to_string();
        entries.insert(
            handle.clone(),
            SnapshotEntry {
                owner: owner.to_owned(),
                bytes,
            },
        );
        Ok(handle)
    }

    pub fn chunk(
        &self,
        handle: &str,
        owner: &str,
        offset: usize,
        requested: usize,
    ) -> Result<Vec<u8>, BrokerError> {
        if requested > MAX_SNAPSHOT_CHUNK {
            return Err(BrokerError::Invalid("snapshot chunk too large".into()));
        }
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        if offset > entry.bytes.len() {
            return Err(BrokerError::Invalid("snapshot offset out of range".into()));
        }
        let end = offset.saturating_add(requested).min(entry.bytes.len());
        Ok(entry.bytes[offset..end].to_vec())
    }

    pub fn size(&self, handle: &str, owner: &str) -> Result<usize, BrokerError> {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        Ok(entry.bytes.len())
    }

    pub fn close(&self, handle: &str, owner: &str) -> Result<(), BrokerError> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = entries
            .get(handle)
            .ok_or_else(|| BrokerError::Invalid("snapshot unavailable".into()))?;
        if entry.owner != owner {
            return Err(BrokerError::Invalid("snapshot owner mismatch".into()));
        }
        entries.remove(handle);
        Ok(())
    }

    pub fn close_owner(&self, owner: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|_, entry| entry.owner != owner);
    }

    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PackageSnapshotFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub fn read_snapshot_tree(
    config: &BrokerConfig,
    root: &Path,
) -> Result<Vec<PackageSnapshotFile>, BrokerError> {
    reject_path(root)?;
    if !config
        .filesystem_roots
        .iter()
        .any(|candidate| path_is_under(candidate, root))
    {
        return Err(BrokerError::Invalid("path escapes configured roots".into()));
    }
    let handle = crate::handle_relative_fs::open_root(root)?;
    let limits = crate::handle_relative_fs::Limits {
        max_bytes_per_file: MAX_BYTES,
        max_total_bytes: MAX_BYTES.saturating_mul(16),
        max_files: MAX_DIRECTORY_ENTRIES,
        max_depth: 64,
    };
    let files = crate::handle_relative_fs::walk_files(&handle, limits).map_err(BrokerError::Io)?;
    Ok(files
        .into_iter()
        .map(|file| PackageSnapshotFile {
            path: file.components.join("/"),
            bytes: file.bytes,
        })
        .collect())
}

pub fn read_file(config: &BrokerConfig, path: &Path) -> Result<Vec<u8>, BrokerError> {
    reject_path(path)?;
    #[cfg(windows)]
    {
        let file = open_existing_target(path)?;
        let metadata = file.metadata()?;
        validate_open_file(config, path, &file, &metadata)?;
        if !metadata.is_file() {
            return Err(BrokerError::Invalid("path is not a regular file".into()));
        }
        let mut bytes = Vec::new();
        file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            return Err(BrokerError::Invalid("file exceeds 1 MiB".into()));
        }
        return Ok(bytes);
    }
    #[cfg(not(windows))]
    let path = path.to_path_buf();
    #[cfg(not(windows))]
    let configured = config
        .filesystem_roots
        .iter()
        .find(|candidate| path_is_under(candidate, &path))
        .ok_or_else(|| BrokerError::Invalid("path escapes configured roots".into()))?;
    #[cfg(not(windows))]
    let relative = path
        .strip_prefix(configured)
        .map_err(|_| BrokerError::Invalid("path escapes configured roots".into()))?;
    #[cfg(not(windows))]
    let components: Vec<&str> = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| BrokerError::Invalid("path is not UTF-8".into()))
        })
        .collect::<Result<_, _>>()?;
    #[cfg(not(windows))]
    crate::handle_relative_fs::read_relative(
        &crate::handle_relative_fs::open_root(configured)?,
        &components,
        MAX_BYTES,
    )
    .map_err(BrokerError::Io)
}

pub fn write_file(config: &BrokerConfig, path: &Path, data: &[u8]) -> Result<(), BrokerError> {
    if data.len() > MAX_BYTES {
        return Err(BrokerError::Invalid("file exceeds 1 MiB".into()));
    }
    reject_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| BrokerError::Invalid("missing parent".into()))?;
    let private_state = config
        .private_state_roots
        .iter()
        .any(|root| path_is_under(root, path));
    let (_parent_guard, parent) = open_parent_dir(config, parent)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| BrokerError::Invalid("missing filename".into()))?,
    );
    let target_guard = match open_existing_target(&target) {
        Ok(file) => {
            let metadata = file.metadata()?;
            validate_open_file(config, &target, &file, &metadata)?;
            if !metadata.is_file() {
                return Err(BrokerError::Invalid("path is not a regular file".into()));
            }
            Some(file)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let (temp_path, mut temp) = create_temp_file(&parent, _parent_guard.as_ref())?;
    let result = (|| -> Result<(), BrokerError> {
        std::io::Write::write_all(&mut temp, data)?;
        temp.sync_all()?;
        if private_state {
            crate::lock_file::apply_owner_only_file_permissions(&temp_path)?;
        }
        if let Some(ref target_handle) = target_guard {
            let metadata = target_handle.metadata()?;
            validate_open_file(config, &target, target_handle, &metadata)?;
        }
        drop(target_guard);
        #[cfg(windows)]
        atomic_replace(
            &temp,
            _parent_guard
                .as_ref()
                .ok_or_else(|| io::Error::other("missing parent handle"))?,
            path.file_name()
                .ok_or_else(|| io::Error::other("missing filename"))?,
        )?;
        #[cfg(not(windows))]
        {
            drop(temp);
            atomic_replace(&temp_path, &target)?;
        }
        Ok(())
    })();
    if result.is_err() {
        #[cfg(windows)]
        {
            // The parent directory may have been renamed after validation.  Delete
            // through the already-open temporary-file handle instead of resolving
            // the canonical path again.
            let _ = delete_temp_file(&temp);
        }
        #[cfg(not(windows))]
        {
            let _ = fs::remove_file(temp_path);
        }
    }
    result
}

pub fn delete_file(config: &BrokerConfig, path: &Path) -> Result<(), BrokerError> {
    reject_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| BrokerError::Invalid("missing parent".into()))?;
    let (_parent_guard, parent) = open_parent_dir(config, parent)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| BrokerError::Invalid("missing filename".into()))?,
    );
    let file = open_existing_target(&target)?;
    let metadata = file.metadata()?;
    validate_open_file(config, &target, &file, &metadata)?;
    if !metadata.is_file() {
        return Err(BrokerError::Invalid("path is not a regular file".into()));
    }
    drop(file);
    fs::remove_file(target)?;
    Ok(())
}

pub fn list_directory(
    config: &BrokerConfig,
    path: &Path,
) -> Result<Vec<DirectoryEntry>, BrokerError> {
    reject_path(path)?;
    let (_guard, canonical) = open_parent_dir(config, path)?;
    let mut entries = Vec::new();
    for item in fs::read_dir(canonical)? {
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            return Err(BrokerError::Invalid("directory exceeds entry bound".into()));
        }
        let item = item?;
        let meta = item.metadata()?;
        if is_reparse_point(&meta) {
            return Err(BrokerError::Invalid("reparse point is not allowed".into()));
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        entries.push(DirectoryEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            kind: if meta.is_dir() {
                "directory".into()
            } else if meta.is_file() {
                "file".into()
            } else {
                "other".into()
            },
            size: meta.len(),
            modified_ms,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn poll_metadata(
    config: &BrokerConfig,
    path: &Path,
) -> Result<Vec<DirectoryEntry>, BrokerError> {
    list_directory(config, path)
}

fn validate_url(config: &BrokerConfig, raw: &str) -> Result<reqwest::Url, BrokerError> {
    let url = reqwest::Url::parse(raw).map_err(|e| BrokerError::Invalid(e.to_string()))?;
    if (url.scheme() != "https"
        && !(cfg!(feature = "package-worker-fixture") && {
            #[cfg(feature = "package-worker-fixture")]
            {
                config.allow_local_test_origin
                    && url.scheme() == "http"
                    && url.host_str().is_some_and(is_loopback_host)
            }
            #[cfg(not(feature = "package-worker-fixture"))]
            {
                false
            }
        }))
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(BrokerError::Invalid(
            "only credential-free HTTPS URLs are allowed".into(),
        ));
    }
    if url
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case("localhost"))
        || !config
            .allowed_origins
            .contains(&url.origin().ascii_serialization())
    {
        return Err(BrokerError::Invalid("origin is not granted".into()));
    }
    Ok(url)
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_multicast()
                || v.is_unspecified()
                || v.octets()[0] == 0
        }
        IpAddr::V6(v) => {
            v.to_ipv4_mapped()
                .is_some_and(|v4| is_blocked_ip(v4.into()))
                || v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || ((v.segments()[0] & 0xfe00) == 0xfc00)
                || ((v.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn reject_path(path: &Path) -> Result<(), BrokerError> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
        || path.to_string_lossy().starts_with("\\\\")
    {
        return Err(BrokerError::Invalid(
            "path must be absolute and traversal-free".into(),
        ));
    }
    Ok(())
}

fn open_existing_target(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options
            .share_mode(0x00000001 | 0x00000002 | 0x00000004)
            .custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

fn open_parent_dir(
    config: &BrokerConfig,
    path: &Path,
) -> Result<(Option<File>, PathBuf), BrokerError> {
    reject_path(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = OpenOptions::new()
            .read(true)
            .write(true)
            // Keep the directory identity open through the replace. Rename is
            // still allowed for the atomic commit itself.
            .share_mode(0x00000001 | 0x00000002 | 0x00000004)
            .access_mode(0x80000000 | 0x40000000 | 0x00000040 | 0x00010000)
            .custom_flags(0x00200000 | 0x02000000) // OPEN_REPARSE_POINT | BACKUP_SEMANTICS
            .open(path)?;
        let metadata = dir.metadata()?;
        if !metadata.is_dir() || is_reparse_point(&metadata) {
            return Err(BrokerError::Invalid("path must be a real directory".into()));
        }
        let canonical = final_path_by_handle(&dir)?;
        if !is_under_configured_root(config, &canonical) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
        return Ok((Some(dir), canonical));
    }
    #[cfg(not(windows))]
    {
        Ok((None, canonical_under_root(config, path)?))
    }
}

fn validate_open_file(
    config: &BrokerConfig,
    _requested: &Path,
    _file: &File,
    metadata: &fs::Metadata,
) -> Result<(), BrokerError> {
    if is_reparse_point(metadata) {
        return Err(BrokerError::Invalid(
            "path must not be a reparse point".into(),
        ));
    }
    #[cfg(windows)]
    {
        let actual = final_path_by_handle(_file)?;
        if !is_under_configured_root(config, &actual) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::canonicalize(_requested)?;
        if !is_under_configured_root(config, &actual) {
            return Err(BrokerError::Invalid("path escapes configured roots".into()));
        }
        let current = fs::metadata(&actual)?;
        if current.dev() != metadata.dev() || current.ino() != metadata.ino() {
            return Err(BrokerError::Invalid(
                "path changed during validation".into(),
            ));
        }
    }
    Ok(())
}

fn is_under_configured_root(config: &BrokerConfig, path: &Path) -> bool {
    config
        .filesystem_roots
        .iter()
        .any(|root| path_is_under(root, path))
}

#[cfg(windows)]
fn path_is_under(root: &Path, path: &Path) -> bool {
    fn normalize(path: &Path) -> String {
        let mut value = path.to_string_lossy().replace('/', "\\");
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            value = rest.to_owned();
        }
        value.trim_end_matches('\\').to_ascii_lowercase()
    }
    let root = normalize(root);
    let path = normalize(path);
    path == root || path.starts_with(&(root + "\\"))
}

#[cfg(not(windows))]
fn path_is_under(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(windows)]
fn final_path_by_handle(file: &File) -> io::Result<PathBuf> {
    use std::{os::windows::ffi::OsStringExt, os::windows::io::AsRawHandle};
    extern "system" {
        fn GetFinalPathNameByHandleW(
            hFile: *mut std::ffi::c_void,
            lpszFilePath: *mut u16,
            cchFilePath: u32,
            dwFlags: u32,
        ) -> u32;
    }
    let handle = file.as_raw_handle();
    let mut buffer = vec![0u16; 512];
    loop {
        let length = unsafe {
            GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, 0)
        };
        if length == 0 {
            return Err(io::Error::last_os_error());
        }
        if (length as usize) < buffer.len() {
            return Ok(PathBuf::from(std::ffi::OsString::from_wide(
                &buffer[..length as usize],
            )));
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}

#[cfg(not(windows))]
fn canonical_under_root(config: &BrokerConfig, path: &Path) -> Result<PathBuf, BrokerError> {
    reject_path(path)?;
    let canonical = std::fs::canonicalize(path)?;
    if config
        .filesystem_roots
        .iter()
        .any(|r| path_is_under(r, &canonical))
    {
        Ok(canonical)
    } else {
        Err(BrokerError::Invalid("path escapes configured roots".into()))
    }
}
fn create_temp_file(
    parent: &Path,
    _parent_handle: Option<&File>,
) -> Result<(PathBuf, File), BrokerError> {
    for _ in 0..MAX_TEMPFILE_ATTEMPTS {
        let suffix = TEMPFILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".package-worker-{}-{suffix}.tmp", std::process::id());
        let path = parent.join(&name);
        #[cfg(windows)]
        let result = {
            let parent_handle =
                _parent_handle.ok_or_else(|| io::Error::other("missing parent handle"))?;
            create_relative_temp_file(parent_handle, std::ffi::OsStr::new(&name))
                .map(|file| (path.clone(), file))
        };
        #[cfg(not(windows))]
        let result = {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            options.open(&path).map(|file| (path.clone(), file))
        };
        match result {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create unique temporary file",
    )
    .into())
}

#[cfg(windows)]
fn create_relative_temp_file(parent: &File, name: &std::ffi::OsStr) -> io::Result<File> {
    use std::{
        os::windows::ffi::OsStrExt,
        os::windows::io::{AsRawHandle, FromRawHandle},
    };

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: *mut std::ffi::c_void,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut std::ffi::c_void,
        security_quality_of_service: *mut std::ffi::c_void,
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    unsafe extern "system" {
        fn NtCreateFile(
            file_handle: *mut *mut std::ffi::c_void,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut std::ffi::c_void,
            ea_length: u32,
        ) -> i32;
    }

    let mut wide: Vec<u16> = name.encode_wide().collect();
    let byte_len = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "temporary name too long"))?;
    if byte_len > u16::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "temporary name too long",
        ));
    }
    let mut unicode = UnicodeString {
        length: byte_len as u16,
        maximum_length: byte_len as u16,
        buffer: wide.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root_directory: parent.as_raw_handle(),
        object_name: &mut unicode,
        attributes: 0x00000040, // OBJ_CASE_INSENSITIVE
        security_descriptor: std::ptr::null_mut(),
        security_quality_of_service: std::ptr::null_mut(),
    };
    let mut status_block = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            0x0013_0116, // generic write + DELETE for handle-relative cleanup
            &mut attributes,
            &mut status_block,
            std::ptr::null_mut(),
            0x00000080,  // FILE_ATTRIBUTE_NORMAL
            0x00000007,  // share read/write/delete
            0x00000002,  // FILE_CREATE
            0x0020_0060, // synchronous, non-directory, open reparse point
            std::ptr::null_mut(),
            0,
        )
    };
    if status == 0xC0000035u32 as i32 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary file already exists",
        ));
    }
    if status < 0 {
        return Err(io::Error::other(format!(
            "NtCreateFile failed: 0x{status:08x}"
        )));
    }
    if handle.is_null() {
        return Err(io::Error::other("NtCreateFile returned a null handle"));
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[cfg(windows)]
fn delete_temp_file(temp: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    #[repr(C)]
    struct FileDispositionInformation {
        delete_file: u8,
    }
    unsafe extern "system" {
        fn NtSetInformationFile(
            file_handle: *mut std::ffi::c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }
    let mut disposition = FileDispositionInformation { delete_file: 1 };
    let mut status_block = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let status = unsafe {
        NtSetInformationFile(
            temp.as_raw_handle(),
            &mut status_block,
            (&mut disposition as *mut FileDispositionInformation).cast(),
            std::mem::size_of::<FileDispositionInformation>() as u32,
            13, // FileDispositionInformation
        )
    };
    if status < 0 {
        return Err(io::Error::other(format!(
            "NtSetInformationFile failed: 0x{status:08x}"
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn atomic_replace(temp: &File, parent: &File, target_name: &std::ffi::OsStr) -> io::Result<()> {
    use std::{
        os::windows::{ffi::OsStrExt, io::AsRawHandle},
        ptr,
    };
    use windows::Win32::Foundation::HANDLE;

    #[repr(C)]
    struct NtFileRenameInfo {
        replace_if_exists: u8,
        root_directory: HANDLE,
        file_name_length: u32,
        file_name: [u16; 1],
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    unsafe extern "system" {
        fn NtSetInformationFile(
            file_handle: HANDLE,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }

    let name: Vec<u16> = target_name.encode_wide().collect();
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing target filename",
        ));
    }
    let size = std::mem::size_of::<NtFileRenameInfo>()
        .checked_add(
            name.len()
                .checked_mul(std::mem::size_of::<u16>())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "target filename too long")
                })?,
        )
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target filename too long"))?;
    let mut info = vec![0u8; size];
    unsafe {
        let rename = info.as_mut_ptr() as *mut NtFileRenameInfo;
        (*rename).replace_if_exists = 1;
        (*rename).root_directory = HANDLE(parent.as_raw_handle() as _);
        (*rename).file_name_length = (name.len() * std::mem::size_of::<u16>()) as u32;
        ptr::copy_nonoverlapping(name.as_ptr(), (*rename).file_name.as_mut_ptr(), name.len());
        let mut io_status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let status = NtSetInformationFile(
            HANDLE(temp.as_raw_handle() as _),
            &mut io_status,
            rename.cast(),
            info.len() as u32,
            10, // FileRenameInformation
        );
        if status < 0 {
            return Err(io::Error::other(format!(
                "NtSetInformationFile failed: 0x{status:08x}"
            )));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(temp: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_private_and_traversal() {
        assert!(is_blocked_ip("127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_blocked_ip("::1".parse().unwrap()));
        assert!(is_blocked_ip("fc00::1".parse().unwrap()));
        assert!(is_blocked_ip("fe80::1".parse().unwrap()));
        assert!(reject_path(Path::new("relative")).is_err());
        assert!(reject_path(Path::new(r"C:\\root\\..\\escape")).is_err());
        assert!(reject_path(Path::new(r"\\server\\share\\file")).is_err());
        assert!(reject_path(Path::new(r"\\.\PIPE\name")).is_err());
    }

    #[test]
    fn validates_https_origin_boundary() {
        let cfg = BrokerConfig::new(["https://example.com"], Vec::new()).unwrap();
        assert!(validate_url(&cfg, "https://example.com/a").is_ok());
        assert!(validate_url(&cfg, "http://example.com/a").is_err());
        assert!(validate_url(&cfg, "https://user@example.com/a").is_err());
        assert!(validate_url(&cfg, "https://example.net/a").is_err());
        assert!(validate_url(&cfg, "https://localhost/a").is_err());
    }
    #[test]
    fn json_session_injection_preserves_worker_body_and_hides_unmapped_fields() {
        let injection: SecretInjection = serde_json::from_value(serde_json::json!({
            "kind": "json", "origins": ["https://example.com"],
            "body_fields": {"token": "accessToken"},
            "header_fields": {"x-huid": "uid"}, "headers": {"x-version": "test"}
        }))
        .unwrap();
        let secret = SecretRequest {
            injection: &injection,
            secret: r#"{"accessToken":"access","uid":"account","refreshToken":"private"}"#,
            allowed_cookie_names: &[],
        };
        let body = serde_json::json!({"version": 42});
        let injected = inject_json_body(Some(&body), Some(&secret))
            .unwrap()
            .unwrap();
        assert_eq!(
            injected,
            serde_json::json!({"version": 42, "token": "access"})
        );
        assert_eq!(body, serde_json::json!({"version": 42}));
        let bytes = request_body(injection.request_method(), Some(&injected))
            .unwrap()
            .unwrap();
        let request = apply_secret(
            reqwest::Client::new()
                .post("https://example.com")
                .body(bytes),
            &secret,
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(request.headers()["x-huid"], "account");
        assert_eq!(request.headers()["x-version"], "test");
        assert!(
            !String::from_utf8_lossy(request.body().unwrap().as_bytes().unwrap())
                .contains("private")
        );
        assert!(inject_json_body(
            Some(&serde_json::json!({"token":"override"})),
            Some(&secret)
        )
        .is_err());
        assert!(inject_json_body(Some(&serde_json::json!([])), Some(&secret)).is_err());
        let missing = SecretRequest {
            secret: r#"{"uid":"account"}"#,
            ..secret
        };
        assert!(inject_json_body(Some(&body), Some(&missing)).is_err());
        let invalid = SecretRequest {
            secret: r#"{"uid":"bad\r\nheader","accessToken":"a"}"#,
            ..missing
        };
        assert!(
            apply_secret(reqwest::Client::new().post("https://example.com"), &invalid).is_err()
        );
    }

    #[test]
    fn network_response_buffers_are_bounded_and_owner_scoped() {
        let responses = SnapshotRegistry::network_responses();
        let data = vec![42; 25 * 1024 * 1024];
        let handle = responses
            .reserve("generation-1", "pkg", "url", data.clone())
            .unwrap();
        assert!(responses
            .reserve("generation-1", "pkg", "url", vec![])
            .is_err());
        assert!(responses.chunk(&handle, "generation-2", 0, 1).is_err());
        assert!(responses.close(&handle, "generation-2").is_err());
        assert!(responses
            .chunk(&handle, "generation-1", 0, MAX_SNAPSHOT_CHUNK + 1)
            .is_err());
        let mut restored = Vec::new();
        while restored.len() < data.len() {
            restored.extend(
                responses
                    .chunk(&handle, "generation-1", restored.len(), MAX_SNAPSHOT_CHUNK)
                    .unwrap(),
            );
        }
        assert_eq!(restored, data);
        for owner in ["b", "c", "d"] {
            responses.reserve(owner, "pkg", "url", vec![]).unwrap();
        }
        assert!(responses.reserve("e", "pkg", "url", vec![]).is_err());
        responses.close_owner("generation-1");
        assert!(responses.chunk(&handle, "generation-1", 0, 1).is_err());
        assert!(responses.reserve("e", "pkg", "url", vec![]).is_ok());
        assert!(SnapshotRegistry::new()
            .reserve("a", "pkg", "url", data)
            .is_err());
    }

    #[cfg(feature = "package-worker-fixture")]
    #[tokio::test]
    async fn network_response_http_limits_reject_oversized_content() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for limit in [MAX_BYTES, MAX_NETWORK_RESPONSE] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                socket.read(&mut request).await.unwrap();
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            limit + 1
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            });
            let config = BrokerConfig::new([origin.clone()], vec![])
                .unwrap()
                .enable_local_test_origin();
            let error = fetch_with_secret_json_limit(&config, &origin, None, None, limit)
                .await
                .unwrap_err();
            assert!(
                matches!(error, BrokerError::Invalid(message) if message == "response exceeds limit")
            );
            server.await.unwrap();
        }
    }

    #[test]
    fn secret_injection_is_manifest_bound_and_filters_cookies() {
        let client = reqwest::Client::new();
        let injection = SecretInjection::Header {
            origins: vec!["https://example.com/".into()],
            name: "Authorization".into(),
            prefix: "Bearer ".into(),
        };
        let request = apply_secret(
            client.get("https://example.com/data"),
            &SecretRequest {
                injection: &injection,
                secret: "token",
                allowed_cookie_names: &[],
            },
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(request.headers()["authorization"], "Bearer token");
        assert!(reqwest::Url::parse(&injection.origins()[0])
            .is_ok_and(|allowed| allowed.origin() == request.url().origin()));

        let cookies = SecretInjection::Cookies {
            origins: vec!["https://example.com/".into()],
            method: IntegrationRequestMethod::PostJson,
            headers: [("origin".into(), "https://example.com".into())].into(),
            header_from_cookie: Some(crate::package_manifest::CookieHeaderInjection {
                cookie: "csrf".into(),
                header: "x-csrf-token".into(),
            }),
        };
        assert!(apply_secret(
            client.get("https://example.com/data"),
            &SecretRequest {
                injection: &cookies,
                secret: r#"{"other":"leak"}"#,
                allowed_cookie_names: &["session".into()],
            },
        )
        .is_err());
        let request = apply_secret(
            client.post("https://example.com/data"),
            &SecretRequest {
                injection: &cookies,
                secret: r#"{"session":"opaque","csrf":"mirror"}"#,
                allowed_cookie_names: &["session".into(), "csrf".into()],
            },
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(request.headers()["origin"], "https://example.com");
        assert_eq!(request.headers()["x-csrf-token"], "mirror");
        assert!(request.headers()["cookie"]
            .to_str()
            .unwrap()
            .contains("session=opaque"));
        assert!(request_body(IntegrationRequestMethod::Get, Some(&serde_json::json!({}))).is_err());
        assert!(request_body(
            IntegrationRequestMethod::PostJson,
            Some(&serde_json::json!({ "body": "x".repeat(MAX_JSON_BODY) }))
        )
        .is_err());
    }
    #[test]
    fn snapshot_lifecycle_returns_opaque_bounded_chunks_and_closes_by_owner() {
        let snapshots = SnapshotRegistry::new();
        let handle = snapshots
            .reserve(
                "desktop-generation-1",
                "eden",
                "bundled",
                b"abcdef".to_vec(),
            )
            .unwrap();
        assert!(!handle.contains('/'));
        assert_eq!(
            snapshots
                .chunk(&handle, "desktop-generation-1", 0, 3)
                .unwrap(),
            b"abc"
        );
        assert_eq!(
            snapshots
                .chunk(&handle, "desktop-generation-1", 3, 3)
                .unwrap(),
            b"def"
        );
        assert!(snapshots.chunk(&handle, "other-generation", 0, 3).is_err());
        snapshots.close_owner("desktop-generation-1");
        assert!(snapshots
            .chunk(&handle, "desktop-generation-1", 0, 3)
            .is_err());
    }

    #[test]
    fn snapshots_real_bytes_and_rejects_nested_symlink() {
        let td = tempfile::tempdir().unwrap();
        let package = td.path().join("pkg");
        fs::create_dir(&package).unwrap();
        fs::write(package.join("manifest.json"), br#"{"id":"pkg"}"#).unwrap();
        fs::create_dir(package.join("dist")).unwrap();
        fs::write(package.join("dist/index.html"), b"immutable").unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
        let files = read_snapshot_tree(&cfg, &package).unwrap();
        assert_eq!(
            files
                .iter()
                .find(|file| file.path == "dist/index.html")
                .unwrap()
                .bytes,
            b"immutable"
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(td.path(), package.join("nested-link")).unwrap();
            assert!(read_snapshot_tree(&cfg, &package).is_err());
        }
    }

    #[test]
    fn reads_and_writes_under_root() {
        let td = tempfile::tempdir().unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
        let nested = td.path().join("nested/deep");
        create_directory(&cfg, &nested).unwrap();
        let p = nested.join("x");
        write_file(&cfg, &p, b"ok").unwrap();
        assert_eq!(read_file(&cfg, &p).unwrap(), b"ok");
        write_file(&cfg, &p, b"second").unwrap();
        assert_eq!(read_file(&cfg, &p).unwrap(), b"second");
        assert!(
            create_directory(&cfg, &tempfile::tempdir().unwrap().path().join("outside")).is_err()
        );
    }

    #[test]
    fn rejects_oversized_and_out_of_root_files() {
        let td = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
        assert!(write_file(&cfg, &td.path().join("large"), &vec![0; MAX_BYTES + 1]).is_err());
        let outside_file = outside.path().join("x");
        fs::write(&outside_file, b"x").unwrap();
        assert!(read_file(&cfg, &outside_file).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let td = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
        let outside_file = outside.path().join("x");
        fs::write(&outside_file, b"x").unwrap();
        let link = td.path().join("link");
        symlink(&outside_file, &link).unwrap();
        assert!(read_file(&cfg, &link).is_err());
        assert!(write_file(&cfg, &link, b"nope").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn detects_path_replacement_after_open() {
        use std::os::unix::fs::MetadataExt;

        let td = tempfile::tempdir().unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
        let path = td.path().join("x");
        fs::write(&path, b"old").unwrap();
        let file = open_existing_target(&path).unwrap();
        let metadata = file.metadata().unwrap();
        fs::rename(&path, td.path().join("old")).unwrap();
        fs::write(&path, b"new").unwrap();
        assert!(validate_open_file(&cfg, &path, &file, &metadata).is_err());
        assert_ne!(metadata.ino(), fs::metadata(&path).unwrap().ino());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn fd_counter_stays_bounded_across_ten_thousand_real_snapshot_cycles() {
        fn fd_count() -> usize {
            std::fs::read_dir("/proc/self/fd").unwrap().count()
        }
        let before = fd_count();
        let snapshots = SnapshotRegistry::new();
        let bytes: Vec<u8> = (0..=255).cycle().take(65_537).collect();
        for _ in 0..10_000 {
            let handle = snapshots
                .reserve("owner", "package", "bundled", bytes.clone())
                .unwrap();
            let mut received = Vec::new();
            let mut offset = 0;
            while offset < bytes.len() {
                let chunk = snapshots
                    .chunk(&handle, "owner", offset, 16 * 1024)
                    .unwrap();
                assert!(!chunk.is_empty());
                received.extend_from_slice(&chunk);
                offset += chunk.len();
            }
            assert_eq!(received, bytes);
            snapshots.close(&handle, "owner").unwrap();
            assert_eq!(snapshots.len(), 0);
        }
        assert!(fd_count().saturating_sub(before) <= 2);
    }

    #[cfg(windows)]
    #[test]
    fn creates_temp_file_relative_to_validated_parent_handle() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let moved = root.path().join("moved");
        fs::create_dir(&original).unwrap();
        let cfg =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![root.path().to_path_buf()]).unwrap();
        let (parent_guard, canonical) = open_parent_dir(&cfg, &original).unwrap();

        // Replace the path after validation.  A path-based CreateFile would
        // follow this new directory; the retained handle must continue to the
        // original directory now reachable through `moved`.
        fs::rename(&original, &moved).unwrap();
        fs::create_dir(&original).unwrap();

        let (temp_path, temp) = create_temp_file(&canonical, parent_guard.as_ref()).unwrap();
        let name = temp_path.file_name().unwrap().to_owned();
        drop(temp);
        assert!(moved.join(&name).is_file());
        assert!(!original.join(&name).exists());
    }
}
