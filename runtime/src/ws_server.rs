use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::agents::AgentsService;
use crate::app_index::AppIndex;
use crate::ark_host::ArkHost;
use crate::auth;
use crate::command_bus::{ClientId, CommandBus, CommandBusEvent, CommandManifest};
use crate::db_backup;
use crate::diagnostics::{RpcDiagnostics, SharedRpcDiagnostics};
use crate::dictation::{handle_dictation_op, DictationHost};
use crate::file_index::{FileIndex, FileIndexSettingsPatch};
use crate::focus::handle_focus_op;
use crate::grant_authority::{GrantAuthorityRegistry, GrantOwner, GrantProvenance};
use crate::integrations;
use crate::manager_api::ManagerState;
use crate::package_service::{PackageError, PackageService};
use crate::package_trust::{SignatureSet, TrustError};
use crate::pomodoro_host::{handle_pomodoro_op, PomodoroHost};
use crate::protocol_usage::{ProtocolUsageStore, TransportKind};
use crate::protocol_version::{Compatibility, ProtocolVersion, API_VERSION, API_VERSION_CURRENT};
use crate::store_catalog::{CatalogDto, PackageIndexLookup, StoreCatalogService};
use crate::usage_tracker::UsageTrackerDiagnosticsState;
use base64::Engine as _;

mod connection;
mod connection_loop;
mod connection_types;
mod dispatch;
mod dispatch_special;
mod dispatch_standard;
mod handshake;
mod lifecycle;
mod ops;
mod server;
mod server_run;
mod shutdown;
#[cfg(test)]
mod tests;
mod types;

use dispatch::dispatch_operation;
pub use handshake::{
    compatibility_label, validate_hello, HelloErrorResponse, HelloMessage, HelloOkResponse,
    HelloOutcome,
};
use ops::*;
pub use server::WsServer;
pub use shutdown::WsShutdownHandle;
use types::*;
pub use types::{handshake_errors, WsServerError};
