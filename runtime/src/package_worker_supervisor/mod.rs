//! Engine-owned, fail-closed stdio supervision for first-party package workers.

#[cfg(windows)]
use crate::package_worker_process::WorkerProcess;
#[cfg(windows)]
use crate::package_worker_process::{LaunchCleanupOwner, WorkerProcessError};
use crate::{
    ark_host::ArkHost,
    observability::{redact_text, BoundedTextTail},
    package_manifest::{IntegrationManifest, IntegrationSettingKind, PackageKind, PackageManifest},
    package_store::PackageStore,
    package_worker_broker::{self, BrokerConfig},
    package_worker_protocol::{
        BootstrapMessage, BridgeStatus, BridgeWorkerConfig, CallMessage, Grant, HeartbeatMessage,
        HelloMessage, IntegrationBootstrapConfig, ResultMessage, RunMessage, WorkerMessage,
        WorkerMethod, MAX_LINE_BYTES,
    },
    package_worker_secrets::PackageWorkerSecretRegistry,
    runtime_grants::{DataRequest, LaunchGrant},
};
use async_trait::async_trait;
use base64::Engine as _;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::future::Future;
#[cfg(windows)]
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
#[cfg(windows)]
use tokio::sync::Mutex as AsyncMutex;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot},
    time,
};

mod authority;
#[cfg(windows)]
mod boxed;
mod calls_dispatch;
mod calls_spawn;
#[cfg(windows)]
mod cleanup;
#[cfg(all(windows, feature = "package-worker-fixture"))]
mod fixtures;
#[cfg(windows)]
mod grant;
mod io;
#[cfg(windows)]
mod launch_prepare;
#[cfg(windows)]
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
#[cfg(windows)]
mod start_initial;
#[cfg(windows)]
mod start_windows;
mod stop;
#[cfg(windows)]
mod tasks;
#[cfg(test)]
mod tests;
mod types;

use calls_dispatch::*;
use calls_spawn::*;
use io::*;
#[cfg(windows)]
use launch_prepare::*;
use lifecycle_finish::*;
use lifecycle_watch::*;
use registry::*;
use retry::*;
#[cfg(windows)]
use tasks::*;
use types::*;

use authority::{dispatch_typed_inner, SupervisorInner};
#[cfg(all(windows, feature = "package-worker-fixture"))]
pub use authority::{AfterLaunchGate, HolderLockGate};
pub use authority::{ArkRequestExecutor, PackageWorkerSupervisor};
pub use types::{IntegrationLaunchConfig, WorkerDiagnostics, WorkerHealth, WorkerState};
