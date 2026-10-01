//! Mundus self-update (KOS-236 "Mundus без Electron"). Ports the update half
//! of `desktop/electron/autoupdater-host.ts` into the Engine: today Mundus
//! Desktop updates via electron-updater running inside the Electron shell,
//! reading `latest.yml` + an NSIS installer from the `makekosmos/desktop`
//! GitHub releases feed. As Electron is removed, the Engine owns this flow
//! instead — see `service.rs` for the state machine and
//! `crate::ws_server::dispatch_standard`'s `updater.` branch for the
//! Manager-only RPC surface (`updater.status` / `check` / `download` /
//! `install`).
//!
//! Kept out of the app-facing RPC surface entirely: `updater.*` operations
//! are only reachable over the Engine's authenticated WebSocket (the
//! `expected_token` handshake in `ws_server::connection`), never through the
//! `/v1/apps/launch/*` HTTP path packaged apps use — `authorize_app_request`
//! in `engine_api/handlers/authorize_request.rs` rejects any operation it
//! does not explicitly recognize, and `updater.*` is not among them. This
//! mirrors how `manager.*` (diagnostics, data browsing, db backups) is
//! already Manager-only in practice.
mod cleanup;
mod download;
mod feed;
mod install;
mod manifest;
mod ops;
mod service;
mod state;
mod version;

pub(crate) use ops::handle_updater_op;
pub use service::UpdaterService;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum UpdaterError {
    #[error("network error: {0}")]
    Network(String),
    #[error("malformed update manifest")]
    MalformedManifest,
    #[error("downloaded file size {actual} does not match expected {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    #[error("downloaded file sha512 does not match the manifest")]
    HashMismatch,
    #[error("filesystem error: {0}")]
    Io(String),
    #[error("silent install is not supported on this platform")]
    UnsupportedPlatform,
    #[error("no update is available to download")]
    NoUpdateAvailable,
    #[error("update has not finished downloading yet")]
    NotDownloaded,
}
