//! Minimal in-process Engine for Android: the desktop Engine's `/v1/rpc`
//! op surface (`get_object`, `list_objects_by_type`, `upsert_object`,
//! `start_sync`, ...) without the HTTP server, the lockfile, or a port.
//! `MobileEngine` hosts `ark_core::service::ArkService` in-process — the
//! same worker-thread service the desktop Engine hosts via `ArkHost`.
//!
//! Kotlin drives it through UniFFI (library mode, no UDL): every method is
//! synchronous, so the app calls it from a background dispatcher and wraps
//! `ChangeListener` callbacks in a `Flow`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ark_core::service::{ArkService, ArkServiceError};
use serde_json::{json, Map, Value};
use tokio::sync::{broadcast, watch};

uniffi::setup_scaffolding!();

/// Errors the host app can see. Handler-rejected ops do not surface here —
/// they stay inside the `{"ok":false,"error":...}` wire shape `call`
/// returns, same as the desktop `/v1/rpc`.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EngineError {
    #[error("failed to open engine: {0}")]
    Open(String),
    #[error("engine is closed")]
    Closed,
    #[error("ark service unavailable: {0}")]
    Unavailable(String),
    #[error("invalid JSON: {0}")]
    BadJson(String),
}

/// Kotlin-implemented callback (`callbackFlow { ... }`). `event_json` is one
/// ark event object — `{"event":"object_upserted","id":...}`,
/// `{"event":"entity_changed","entity":...}`, `peer_connected`, ...
#[uniffi::export(callback_interface)]
pub trait ChangeListener: Send + Sync {
    fn on_change(&self, event_json: String);
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread tokio runtime")
        .block_on(future)
}

/// Serialize one `ArkService::call` into the same `{ok,data,error}` wire
/// shape the desktop `/v1/rpc` returns.
fn call_json(service: &ArkService, op: &str, params: Value) -> Result<String, EngineError> {
    let response = match block_on(service.call(op, params)) {
        Ok(data) => json!({"ok": true, "data": data, "error": Value::Null}),
        Err(ArkServiceError::Request(error)) => {
            json!({"ok": false, "data": Value::Null, "error": error})
        }
        Err(error) => return Err(EngineError::Unavailable(error.to_string())),
    };
    Ok(response.to_string())
}

/// Stable per-install sync identity, same convention as the desktop Engine
/// (`resolve_device_id` in runtime/src/sync.rs): read `mundus-device-id.txt`
/// inside the data dir, create it on first start.
fn resolve_device_id(data_dir: &Path) -> Result<String, EngineError> {
    let id_path = data_dir.join("mundus-device-id.txt");
    if let Ok(raw) = std::fs::read_to_string(&id_path) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_owned());
        }
    }
    let generated: String = uuid::Uuid::new_v4().simple().to_string()[..16].to_owned();
    std::fs::create_dir_all(data_dir).map_err(|e| EngineError::Open(e.to_string()))?;
    std::fs::write(&id_path, &generated).map_err(|e| EngineError::Open(e.to_string()))?;
    Ok(generated)
}

/// The engine object the Kotlin app holds for the process lifetime.
/// Drop order matters: `shutdown()` (or dropping the object) shuts the ark
/// worker down before the process moves on.
#[derive(uniffi::Object)]
pub struct MobileEngine {
    data_dir: PathBuf,
    service: Mutex<Option<ArkService>>,
}

impl MobileEngine {
    fn service(&self) -> Result<MutexGuard<'_, Option<ArkService>>, EngineError> {
        Ok(self.service.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn call_service(&self, op: &str, params: Value) -> Result<String, EngineError> {
        let guard = self.service()?;
        let service = guard.as_ref().ok_or(EngineError::Closed)?;
        call_json(service, op, params)
    }
}

#[uniffi::export]
impl MobileEngine {
    /// `data_dir` is `Context.filesDir` (or a subdir) passed by the app.
    /// The database lives at `<data_dir>/ark.db` — the same filename the
    /// desktop uses inside its data dir.
    #[uniffi::constructor]
    pub fn open(data_dir: String) -> Result<Arc<Self>, EngineError> {
        let data_dir = PathBuf::from(data_dir);
        std::fs::create_dir_all(&data_dir).map_err(|e| EngineError::Open(e.to_string()))?;
        let db_path = data_dir.join("ark.db").to_string_lossy().into_owned();
        let service =
            block_on(ArkService::open(db_path)).map_err(|e| EngineError::Open(e.to_string()))?;
        Ok(Arc::new(Self {
            data_dir,
            service: Mutex::new(Some(service)),
        }))
    }

    /// Thin pass-through to the ark op surface. `params_json` is the op's
    /// parameter object; returns the `{ok,data,error}` response as JSON.
    pub fn call(&self, op: String, params_json: String) -> Result<String, EngineError> {
        let params: Value =
            serde_json::from_str(&params_json).map_err(|e| EngineError::BadJson(e.to_string()))?;
        self.call_service(&op, params)
    }

    /// Feed the ark event bus into a `ChangeListener`. Each subscription
    /// owns a pump thread; dropping the returned handle stops it.
    pub fn subscribe(
        &self,
        listener: Box<dyn ChangeListener>,
    ) -> Result<Arc<ChangeSubscription>, EngineError> {
        let receiver = self
            .service()?
            .as_ref()
            .ok_or(EngineError::Closed)?
            .subscribe();
        Ok(ChangeSubscription::start(receiver, listener))
    }

    /// Start sync toward a desktop peer. `config_json` accepts the
    /// `start_sync` params (`space_id`, `device_name`, `use_iroh`,
    /// `iroh_peer_ticket`, `relay_url`, `seed_addresses`, `bind`, ...);
    /// missing keys get mobile defaults (`space_id` = "mundus-default",
    /// `device_id` = persisted per-install id, `use_iroh` = true).
    pub fn start_sync(&self, config_json: String) -> Result<String, EngineError> {
        let mut params: Map<String, Value> = match serde_json::from_str(&config_json)
            .map_err(|e| EngineError::BadJson(e.to_string()))?
        {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            _ => return Err(EngineError::BadJson("config must be an object".into())),
        };
        params
            .entry("space_id")
            .or_insert_with(|| json!("mundus-default"));
        if !params.contains_key("device_id") {
            params.insert(
                "device_id".into(),
                json!(resolve_device_id(&self.data_dir)?),
            );
        }
        params.entry("port").or_insert(Value::Null);
        params.entry("seed_addresses").or_insert(Value::Null);
        params.entry("use_iroh").or_insert_with(|| json!(true));
        params
            .entry("app_version")
            .or_insert_with(|| json!(concat!("engine-mobile/", env!("CARGO_PKG_VERSION"))));
        self.call_service("start_sync", Value::Object(params))
    }

    pub fn stop_sync(&self) -> Result<String, EngineError> {
        self.call_service("stop_sync", json!({}))
    }

    /// `get_sync_snapshot` — running state, peers, transport choice.
    pub fn sync_status(&self) -> Result<String, EngineError> {
        self.call_service("get_sync_snapshot", json!({}))
    }

    /// Escalate to a LAN-reachable bind and return our iroh pairing ticket
    /// (the desktop-side user action for pairing; `show_pairing_code` in
    /// ark terms).
    pub fn show_pairing_code(&self) -> Result<String, EngineError> {
        self.call_service("show_pairing_code", json!({}))
    }

    /// Passive read of our iroh ticket — `null` inside `data` until sync is
    /// running with the iroh transport.
    pub fn own_iroh_ticket(&self) -> Result<String, EngineError> {
        self.call_service("get_own_iroh_ticket", json!({}))
    }

    /// Pair with a peer by its ticket/pairing code.
    pub fn connect_with_pairing_code(&self, code: String) -> Result<String, EngineError> {
        self.call_service("connect_with_pairing_code", json!({"pairing_code": code}))
    }

    /// Shut the ark worker down. Idempotent; further calls return
    /// `EngineError::Closed`. Named `shutdown` rather than `close` so the
    /// generated Kotlin `AutoCloseable.close()` destructor stays unique.
    pub fn shutdown(&self) {
        let _ = self.service().map(|mut guard| guard.take());
    }
}

/// Handle for one `subscribe` registration. Drop stops the pump thread.
#[derive(uniffi::Object)]
pub struct ChangeSubscription {
    stop: watch::Sender<bool>,
    pump: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ChangeSubscription {
    fn start(
        mut receiver: broadcast::Receiver<Value>,
        listener: Box<dyn ChangeListener>,
    ) -> Arc<Self> {
        let (stop, mut stop_rx) = watch::channel(false);
        let pump = std::thread::Builder::new()
            .name("engine-mobile-events".into())
            .spawn(move || {
                block_on(async move {
                    loop {
                        tokio::select! {
                            _ = stop_rx.changed() => break,
                            message = receiver.recv() => match message {
                                Ok(event) => listener.on_change(event.to_string()),
                                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                                Err(broadcast::error::RecvError::Closed) => break,
                            },
                        }
                    }
                });
            })
            .expect("spawn event pump thread");
        Arc::new(Self {
            stop,
            pump: Mutex::new(Some(pump)),
        })
    }
}

impl Drop for ChangeSubscription {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        if let Some(pump) = self.pump.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = pump.join();
        }
    }
}
