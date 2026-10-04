//! Engine-owned, fail-closed stdio supervision for first-party package workers.

#[cfg(any(windows, target_os = "macos"))]
use crate::package_worker_process::WorkerProcess;
#[cfg(any(windows, target_os = "macos"))]
use crate::package_worker_process::{LaunchCleanupOwner, WorkerProcessError};
use crate::{
    ark_host::ArkHost,
    grant_authority::{GrantAuthorityRegistry, GrantOwner},
    observability::{redact_text, BoundedTextTail},
    package_manifest::{IntegrationManifest, PackageKind, PackageManifest},
    package_store::PackageStore,
    package_worker_broker::{self, BrokerConfig},
    package_worker_protocol::{
        BridgeStatus, BridgeWorkerConfig, CallMessage, Grant, HeartbeatMessage, HelloMessage,
        InvokeMessage, ResultMessage, RunMessage, WorkerMessage, WorkerMethod, MAX_LINE_BYTES,
    },
    package_worker_secrets::PackageWorkerSecretRegistry,
    runtime_grants::{DataRequest, LaunchGrant},
};
use async_trait::async_trait;
use base64::Engine as _;
use sha2::{Digest, Sha256};
#[cfg(any(windows, target_os = "macos"))]
use std::future::Future;
#[cfg(any(windows, target_os = "macos"))]
use std::pin::Pin;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[cfg(any(windows, target_os = "macos"))]
use tokio::io::{AsyncWrite, AsyncWriteExt};
#[cfg(any(windows, target_os = "macos"))]
use tokio::sync::Mutex as AsyncMutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt, BufReader},
    sync::{mpsc, oneshot},
    time,
};

mod authority;
#[cfg(any(windows, target_os = "macos"))]
mod boxed;
mod calls_dispatch;
mod calls_spawn;
#[cfg(any(windows, target_os = "macos"))]
mod cleanup;
#[path = "authority/engine_capability.rs"]
mod engine_capability;
#[cfg(all(windows, feature = "package-worker-fixture"))]
mod fixtures;
#[cfg(any(windows, target_os = "macos"))]
mod grant;
mod grant_authority;
mod io;
#[cfg(any(windows, target_os = "macos"))]
mod launch_prepare;
#[cfg(any(windows, target_os = "macos"))]
mod launch_transaction;
mod lifecycle_finish;
mod lifecycle_watch;
mod misc;
mod public_api;
mod registry;
mod registry_reap;
mod registry_shutdown;
mod retry;
mod shutdown;
#[cfg(any(windows, target_os = "macos"))]
mod start_initial;
#[cfg(any(windows, target_os = "macos"))]
mod start_windows;
mod stop;
#[cfg(any(windows, target_os = "macos"))]
mod tasks;
#[cfg(test)]
mod tests;
mod types;

use calls_dispatch::*;
use calls_spawn::*;
use io::*;
#[cfg(any(windows, target_os = "macos"))]
use launch_prepare::*;
#[cfg(any(windows, target_os = "macos", test))]
use lifecycle_finish::*;
use lifecycle_watch::*;
use registry::*;
use retry::*;

#[cfg(any(windows, target_os = "macos"))]
use crate::package_worker_protocol::{BootstrapMessage, IntegrationBootstrapConfig};
#[cfg(any(windows, target_os = "macos"))]
use tasks::*;
use types::*;

use authority::{dispatch_typed_inner, SupervisorInner};
#[cfg(all(windows, feature = "package-worker-fixture"))]
pub use authority::{AfterLaunchGate, HolderLockGate};
#[cfg(all(windows, feature = "package-worker-fixture"))]
use authority::{AfterLaunchGateParts, NEXT_AFTER_LAUNCH_GATE};
pub use authority::{ArkRequestExecutor, PackageWorkerSupervisor};
pub use engine_capability::{AutostartControl, EngineCapabilityExecutor};
pub use types::{IntegrationLaunchConfig, WorkerDiagnostics, WorkerHealth, WorkerState};
