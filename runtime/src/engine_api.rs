use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::HeaderMap;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::sync::Notify;

use crate::auth;
use crate::engine_dispatch::{DispatchClient, DispatchRequest, Operation};
use crate::package_service::PackageService;
use crate::protocol_usage::ProtocolUsageStore;
use crate::protocol_version::{
    Compatibility, ProtocolVersion, API_VERSION, API_VERSION_CURRENT, PROTOCOL_VERSION,
};
use crate::runtime_grants::{DataRequest, FieldInput, LaunchGrant};

// 32 MiB — `dictation.speech.transcribe` ships base64 WAV in the JSON body
// (~43 KiB/s at 16 kHz mono), so the old 1 MiB cap 413'd any recording
// longer than ~24 s. 32 MiB covers ~12 min of dictation.
const MAX_HTTP_BODY_BYTES: usize = 32 * 1024 * 1024;
const LAUNCH_LEASE_TTL: Duration = Duration::from_secs(300);
const DATA_GRANT_TTL: Duration = Duration::from_secs(900);
const MAX_ACTIVE_LAUNCH_LEASES: usize = 2_048;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
// Wire header names stay `x-kosmos-*`: pinned component builds
// (agenda/memoria/dictation and kosmos-gpui-kit) send exactly these names —
// persisted contract, see docs/brand-legacy-identifiers.md.
const CLIENT_PID_HEADER: &str = "x-kosmos-client-pid";
const API_VERSION_HEADER: &str = "x-kosmos-api-version";
const CLIENT_CLASS_HEADER: &str = "x-kosmos-client-class";
const CLIENT_VERSION_HEADER: &str = "x-kosmos-client-version";
const APP_LAUNCH_TOKEN_HEADER: &str = "x-kosmos-launch-token";

type HttpResponse = Response<Full<Bytes>>;

const MAX_IN_FLIGHT_HTTP_OPERATIONS: usize = 128;
const MAX_ACTIVE_HTTP_CONNECTIONS: usize = 128;
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(5);

include!("engine_api/lifecycle.rs");
include!("engine_api/server.rs");
include!("engine_api/handlers.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    include!("engine_api/tests_core.rs");
    include!("engine_api/tests_http.rs");
}
