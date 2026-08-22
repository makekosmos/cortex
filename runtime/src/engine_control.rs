use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashSet;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{timeout, Duration};

pub const CONTROL_PROTOCOL_VERSION: u16 = 1;
pub const MAX_CONTROL_FRAME_BYTES: usize = 16 * 1024;
pub const MAX_CONTROL_REQUEST_ID_BYTES: usize = 128;
pub const MAX_CONTROL_OWNER_ID_BYTES: usize = 128;
pub const MAX_CONTROL_SEEN_REQUESTS: usize = 1024;
pub const CONTROL_IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlMessage {
    RestartRequested,
    ShutdownRequested,
    CoreReady,
    CoreStopping,
    DesktopLease {
        electron_pid: u32,
        credential: String,
    },
    DesktopLeaseRevoked {
        generation: u64,
    },
    DesktopLeaseInstalled {
        generation: u64,
        electron_pid: u32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlChannel {
    Controller,
    CoreEvents,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlEnvelope {
    pub protocol_version: u16,
    pub supervisor_session_id: String,
    pub child_generation: u64,
    pub request_id: String,
    pub owner_identity: String,
    pub channel: ControlChannel,
    pub message: ControlMessage,
    pub authentication: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPhase {
    AwaitingReady,
    Running,
    StopRequested,
    Stopped,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ControlError {
    #[error("control_malformed")]
    Malformed,
    #[error("control_unauthorized")]
    Unauthorized,
    #[error("control_stale_generation")]
    StaleGeneration,
    #[error("control_replayed")]
    Replayed,
    #[error("control_out_of_order")]
    OutOfOrder,
    #[error("control_timeout")]
    Timeout,
    #[error("supervisor_unavailable")]
    Unavailable,
    #[error("control_io")]
    Io,
}

pub struct ControlSession {
    session_id: String,
    generation: u64,
    owner_identity: String,
    controller_secret: Vec<u8>,
    core_secret: Vec<u8>,
    phase: ControlPhase,
    seen_requests: HashSet<String>,
}

impl ControlSession {
    pub fn new(
        session_id: impl Into<String>,
        generation: u64,
        owner_identity: impl Into<String>,
        controller_secret: &[u8],
        core_secret: &[u8],
    ) -> Self {
        Self {
            session_id: session_id.into(),
            generation,
            owner_identity: owner_identity.into(),
            controller_secret: controller_secret.to_vec(),
            core_secret: core_secret.to_vec(),
            phase: ControlPhase::AwaitingReady,
            seen_requests: HashSet::new(),
        }
    }

    pub fn phase(&self) -> ControlPhase {
        self.phase
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn authenticate(&self, envelope: &ControlEnvelope) -> bool {
        let secret = match envelope.channel {
            ControlChannel::Controller => &self.controller_secret,
            ControlChannel::CoreEvents => &self.core_secret,
        };
        constant_time_equal(
            authentication_tag(secret, envelope).as_bytes(),
            envelope.authentication.as_bytes(),
        )
    }

    pub fn validate(&mut self, envelope: &ControlEnvelope) -> Result<ControlMessage, ControlError> {
        validate_bounds(envelope)?;
        if envelope.protocol_version != CONTROL_PROTOCOL_VERSION
            || envelope.supervisor_session_id != self.session_id
            || envelope.owner_identity != self.owner_identity
            || !self.authenticate(envelope)
        {
            return Err(ControlError::Unauthorized);
        }
        if envelope.child_generation != self.generation {
            return Err(ControlError::StaleGeneration);
        }
        let role_allowed = matches!(
            (&envelope.channel, &envelope.message),
            (
                ControlChannel::Controller,
                ControlMessage::RestartRequested
                    | ControlMessage::ShutdownRequested
                    | ControlMessage::DesktopLease { .. }
                    | ControlMessage::DesktopLeaseRevoked { .. },
            ) | (
                ControlChannel::CoreEvents,
                ControlMessage::CoreReady
                    | ControlMessage::CoreStopping
                    | ControlMessage::DesktopLeaseInstalled { .. },
            )
        );
        if !role_allowed {
            return Err(ControlError::Unauthorized);
        }
        if !self.seen_requests.insert(envelope.request_id.clone()) {
            return Err(ControlError::Replayed);
        }
        let allowed = matches!(
            (&envelope.message, self.phase),
            (ControlMessage::CoreReady, ControlPhase::AwaitingReady)
                | (
                    ControlMessage::RestartRequested | ControlMessage::ShutdownRequested,
                    ControlPhase::Running,
                )
                | (
                    ControlMessage::DesktopLease { .. }
                        | ControlMessage::DesktopLeaseRevoked { .. },
                    ControlPhase::Running,
                )
                | (
                    ControlMessage::DesktopLeaseInstalled { .. },
                    ControlPhase::Running
                )
                | (ControlMessage::CoreStopping, ControlPhase::StopRequested)
        );
        if !allowed {
            return Err(ControlError::OutOfOrder);
        }
        self.phase = match envelope.message {
            ControlMessage::CoreReady => ControlPhase::Running,
            ControlMessage::RestartRequested | ControlMessage::ShutdownRequested => {
                ControlPhase::StopRequested
            }
            ControlMessage::CoreStopping => ControlPhase::Stopped,
            ControlMessage::DesktopLease { .. }
            | ControlMessage::DesktopLeaseRevoked { .. }
            | ControlMessage::DesktopLeaseInstalled { .. } => self.phase,
        };
        if self.seen_requests.len() > MAX_CONTROL_SEEN_REQUESTS {
            if let Some(first) = self.seen_requests.iter().next().cloned() {
                self.seen_requests.remove(&first);
            }
        }
        Ok(envelope.message.clone())
    }
}

fn validate_bounds(envelope: &ControlEnvelope) -> Result<(), ControlError> {
    if envelope.request_id.is_empty()
        || envelope.request_id.len() > MAX_CONTROL_REQUEST_ID_BYTES
        || envelope.supervisor_session_id.is_empty()
        || envelope.owner_identity.is_empty()
        || envelope.owner_identity.len() > MAX_CONTROL_OWNER_ID_BYTES
        || envelope.authentication.len() != 64
        || serde_json::to_vec(envelope)
            .map(|v| v.len() > MAX_CONTROL_FRAME_BYTES)
            .unwrap_or(true)
    {
        return Err(ControlError::Malformed);
    }
    Ok(())
}

pub fn authentication_tag(secret: &[u8], envelope: &ControlEnvelope) -> String {
    let mut unsigned = envelope.clone();
    unsigned.authentication.clear();
    let payload = serde_json::to_vec(&unsigned).expect("control envelope is serializable");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&payload);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlState {
    pub endpoint: String,
    pub supervisor_session_id: String,
    pub child_generation: u64,
    pub owner_identity: String,
    pub secret: String,
}

#[derive(Debug)]
pub struct ControlServer {
    address: String,
    session_id: String,
    owner_identity: String,
    generation: u64,
    events: mpsc::Receiver<ControlMessage>,
    task: tokio::task::JoinHandle<()>,
    core_commands: Arc<Mutex<Option<CoreCommandConnection>>>,
}

#[derive(Debug, Clone)]
struct CoreCommandConnection {
    id: u64,
    generation: u64,
    sender: mpsc::Sender<ControlMessage>,
}

pub struct CoreCommandReceiver {
    receiver: mpsc::Receiver<ControlMessage>,
    task: JoinHandle<()>,
    session_id: String,
    generation: u64,
}

impl CoreCommandReceiver {
    pub async fn recv(&mut self) -> Option<ControlMessage> {
        self.receiver.recv().await
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl Drop for CoreCommandReceiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl ControlServer {
    pub async fn bind(
        session_id: String,
        generation: u64,
        owner_identity: String,
        controller_secret: Vec<u8>,
        core_secret: Vec<u8>,
    ) -> Result<(Self, String), ControlError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| ControlError::Unavailable)?;
        let address = listener
            .local_addr()
            .map_err(|_| ControlError::Unavailable)?
            .to_string();
        let session_id_for_server = session_id.clone();
        let owner_identity_for_server = owner_identity.clone();
        let state = Arc::new(Mutex::new(ControlSession::new(
            session_id,
            generation,
            owner_identity,
            &controller_secret,
            &core_secret,
        )));
        let (sender, events) = mpsc::channel(16);
        let core_commands = Arc::new(Mutex::new(None));
        let next_connection_id = Arc::new(AtomicU64::new(0));
        let task = tokio::spawn(accept_loop(
            listener,
            state,
            sender,
            Arc::clone(&core_commands),
            next_connection_id,
        ));
        Ok((
            Self {
                address: address.clone(),
                session_id: session_id_for_server,
                owner_identity: owner_identity_for_server,
                generation,
                events,
                task,
                core_commands,
            },
            address,
        ))
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn owner_identity(&self) -> &str {
        &self.owner_identity
    }

    pub fn child_generation(&self) -> u64 {
        self.generation
    }

    pub async fn recv(&mut self) -> Option<ControlMessage> {
        self.events.recv().await
    }

    pub fn abort(&self) {
        self.task.abort();
    }

    pub async fn send_to_core(&self, message: ControlMessage) -> Result<(), ControlError> {
        let sender = self
            .core_commands
            .lock()
            .await
            .clone()
            .map(|connection| connection.sender);
        sender
            .ok_or(ControlError::Unavailable)?
            .send(message)
            .await
            .map_err(|_| ControlError::Unavailable)
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn accept_loop(
    listener: TcpListener,
    state: Arc<Mutex<ControlSession>>,
    sender: mpsc::Sender<ControlMessage>,
    core_commands: Arc<Mutex<Option<CoreCommandConnection>>>,
    next_connection_id: Arc<AtomicU64>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { break };
                let state = Arc::clone(&state);
                let sender = sender.clone();
                let core_commands = Arc::clone(&core_commands);
                let connection_id = next_connection_id.fetch_add(1, Ordering::Relaxed) + 1;
                connections.spawn(async move {
                    let _ = handle_connection(
                        stream,
                        state,
                        sender,
                        core_commands,
                        connection_id,
                    ).await;
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                let _ = result;
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<ControlSession>>,
    sender: mpsc::Sender<ControlMessage>,
    core_commands: Arc<Mutex<Option<CoreCommandConnection>>>,
    connection_id: u64,
) -> Result<(), ControlError> {
    let result = timeout(CONTROL_IO_TIMEOUT, read_envelope(&mut stream)).await;
    match result {
        Ok(Ok(envelope)) => {
            let message = {
                let mut guard = state.lock().await;
                match guard.validate(&envelope) {
                    Ok(message) => message,
                    Err(error) => {
                        write_response(&mut stream, &ControlResponse::error(error)).await?;
                        return Ok(());
                    }
                }
            };
            if message == ControlMessage::CoreReady {
                let _ = sender.send(message).await;
                write_response(&mut stream, &ControlResponse::accepted()).await?;
                let (commands_tx, mut commands_rx) = mpsc::channel(4);
                *core_commands.lock().await = Some(CoreCommandConnection {
                    id: connection_id,
                    generation: envelope.child_generation,
                    sender: commands_tx,
                });
                let mut peer_probe = [0u8; 1];
                loop {
                    tokio::select! {
                        command = commands_rx.recv() => {
                            let Some(command) = command else { break };
                            if write_command(&mut stream, command).await.is_err() {
                                break;
                            }
                        }
                        peer = stream.read(&mut peer_probe) => {
                            match peer {
                                Ok(0) | Err(_) => break,
                                Ok(_) => return Err(ControlError::Malformed),
                            }
                        }
                    }
                }
                clear_core_commands(&core_commands, connection_id, envelope.child_generation).await;
                return Ok(());
            }
            if matches!(
                message,
                ControlMessage::RestartRequested
                    | ControlMessage::ShutdownRequested
                    | ControlMessage::DesktopLease { .. }
                    | ControlMessage::DesktopLeaseRevoked { .. }
            ) {
                if let Some(core_sender) = core_commands.lock().await.clone() {
                    let _ = core_sender.sender.send(message.clone()).await;
                }
            }
            let _ = sender.send(message).await;
            write_response(&mut stream, &ControlResponse::accepted()).await
        }
        Ok(Err(error)) => write_response(&mut stream, &ControlResponse::error(error)).await,
        Err(_) => write_response(&mut stream, &ControlResponse::error(ControlError::Timeout)).await,
    }
}

async fn clear_core_commands(
    core_commands: &Arc<Mutex<Option<CoreCommandConnection>>>,
    connection_id: u64,
    generation: u64,
) {
    let mut current = core_commands.lock().await;
    if current.as_ref().is_some_and(|connection| {
        connection.id == connection_id && connection.generation == generation
    }) {
        *current = None;
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ControlResponse {
    accepted: bool,
    error: Option<String>,
}

impl ControlResponse {
    fn accepted() -> Self {
        Self {
            accepted: true,
            error: None,
        }
    }

    fn error(error: ControlError) -> Self {
        Self {
            accepted: false,
            error: Some(error.to_string()),
        }
    }
}

async fn read_envelope(stream: &mut TcpStream) -> Result<ControlEnvelope, ControlError> {
    let size = stream.read_u32().await.map_err(|_| ControlError::Io)? as usize;
    if size == 0 || size > MAX_CONTROL_FRAME_BYTES {
        return Err(ControlError::Malformed);
    }
    let mut bytes = vec![0u8; size];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ControlError::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| ControlError::Malformed)
}

async fn write_command(
    stream: &mut TcpStream,
    message: ControlMessage,
) -> Result<(), ControlError> {
    let bytes = serde_json::to_vec(&message).map_err(|_| ControlError::Io)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ControlError::Malformed);
    }
    stream
        .write_u32(bytes.len() as u32)
        .await
        .map_err(|_| ControlError::Io)?;
    stream.write_all(&bytes).await.map_err(|_| ControlError::Io)
}

async fn read_command(stream: &mut TcpStream) -> Result<ControlMessage, ControlError> {
    let size = stream.read_u32().await.map_err(|_| ControlError::Io)? as usize;
    if size == 0 || size > MAX_CONTROL_FRAME_BYTES {
        return Err(ControlError::Malformed);
    }
    let mut bytes = vec![0u8; size];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ControlError::Io)?;
    serde_json::from_slice(&bytes).map_err(|_| ControlError::Malformed)
}

async fn write_response(
    stream: &mut TcpStream,
    response: &ControlResponse,
) -> Result<(), ControlError> {
    let bytes = serde_json::to_vec(response).map_err(|_| ControlError::Io)?;
    timeout(CONTROL_IO_TIMEOUT, async {
        stream
            .write_u32(bytes.len() as u32)
            .await
            .map_err(|_| ControlError::Io)?;
        stream.write_all(&bytes).await.map_err(|_| ControlError::Io)
    })
    .await
    .map_err(|_| ControlError::Timeout)??;
    Ok(())
}

pub async fn send_control(
    state: &ControlState,
    message: ControlMessage,
) -> Result<(), ControlError> {
    if matches!(
        message,
        ControlMessage::CoreReady
            | ControlMessage::CoreStopping
            | ControlMessage::DesktopLeaseInstalled { .. }
    ) {
        return Err(ControlError::Unauthorized);
    }
    send_control_with_key(state, message, ControlChannel::Controller, None).await
}

async fn send_control_with_key(
    state: &ControlState,
    message: ControlMessage,
    channel: ControlChannel,
    key_override: Option<&[u8]>,
) -> Result<(), ControlError> {
    if !state.endpoint.starts_with("127.0.0.1:") {
        return Err(ControlError::Unavailable);
    }
    let mut stream = timeout(CONTROL_IO_TIMEOUT, TcpStream::connect(&state.endpoint))
        .await
        .map_err(|_| ControlError::Timeout)?
        .map_err(|_| ControlError::Unavailable)?;
    let secret = match key_override {
        Some(secret) => secret.to_vec(),
        None => decode_secret(&state.secret)?,
    };
    let mut envelope = ControlEnvelope {
        protocol_version: CONTROL_PROTOCOL_VERSION,
        supervisor_session_id: state.supervisor_session_id.clone(),
        child_generation: state.child_generation,
        request_id: uuid::Uuid::new_v4().to_string(),
        owner_identity: state.owner_identity.clone(),
        channel,
        message,
        authentication: String::new(),
    };
    envelope.authentication = authentication_tag(&secret, &envelope);
    let bytes = serde_json::to_vec(&envelope).map_err(|_| ControlError::Malformed)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ControlError::Malformed);
    }
    timeout(CONTROL_IO_TIMEOUT, async {
        stream
            .write_u32(bytes.len() as u32)
            .await
            .map_err(|_| ControlError::Io)?;
        stream
            .write_all(&bytes)
            .await
            .map_err(|_| ControlError::Io)?;
        let size = stream.read_u32().await.map_err(|_| ControlError::Io)? as usize;
        if size == 0 || size > MAX_CONTROL_FRAME_BYTES {
            return Err(ControlError::Malformed);
        }
        let mut response = vec![0u8; size];
        stream
            .read_exact(&mut response)
            .await
            .map_err(|_| ControlError::Io)?;
        let response: ControlResponse =
            serde_json::from_slice(&response).map_err(|_| ControlError::Malformed)?;
        if response.accepted {
            Ok(())
        } else {
            Err(match response.error.as_deref() {
                Some("control_unauthorized") => ControlError::Unauthorized,
                Some("control_stale_generation") => ControlError::StaleGeneration,
                Some("control_replayed") => ControlError::Replayed,
                Some("control_out_of_order") => ControlError::OutOfOrder,
                Some("control_timeout") => ControlError::Timeout,
                _ => ControlError::Io,
            })
        }
    })
    .await
    .map_err(|_| ControlError::Timeout)??;
    Ok(())
}

pub fn send_control_blocking(
    state: &ControlState,
    message: ControlMessage,
) -> Result<(), ControlError> {
    let state = state.clone();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(|_| ControlError::Unavailable)?
            .block_on(send_control(&state, message))
    })
    .join()
    .map_err(|_| ControlError::Unavailable)?
}

pub fn state_from_env() -> Result<ControlState, ControlError> {
    let endpoint =
        std::env::var("KOSMOS_CONTROL_ENDPOINT").map_err(|_| ControlError::Unavailable)?;
    let supervisor_session_id =
        std::env::var("KOSMOS_CONTROL_SESSION_ID").map_err(|_| ControlError::Unavailable)?;
    let child_generation = std::env::var("KOSMOS_CONTROL_GENERATION")
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or(ControlError::Malformed)?;
    let owner_identity =
        std::env::var("KOSMOS_CONTROL_OWNER_ID").map_err(|_| ControlError::Unavailable)?;
    let secret =
        std::env::var("KOSMOS_CORE_CONTROL_SECRET").map_err(|_| ControlError::Unavailable)?;
    Ok(ControlState {
        endpoint,
        supervisor_session_id,
        child_generation,
        owner_identity,
        secret,
    })
}

pub async fn start_core_control() -> Result<CoreCommandReceiver, ControlError> {
    let state = state_from_env()?;
    let core_secret = decode_secret(&state.secret)?;
    start_core_control_with_secret(state, core_secret).await
}

pub async fn start_core_control_with_secret(
    state: ControlState,
    core_secret: Vec<u8>,
) -> Result<CoreCommandReceiver, ControlError> {
    if !state.endpoint.starts_with("127.0.0.1:") {
        return Err(ControlError::Unavailable);
    }
    let mut stream = timeout(CONTROL_IO_TIMEOUT, TcpStream::connect(&state.endpoint))
        .await
        .map_err(|_| ControlError::Timeout)?
        .map_err(|_| ControlError::Unavailable)?;
    let session_id = state.supervisor_session_id.clone();
    let generation = state.child_generation;
    let mut envelope = ControlEnvelope {
        protocol_version: CONTROL_PROTOCOL_VERSION,
        supervisor_session_id: session_id.clone(),
        child_generation: generation,
        request_id: uuid::Uuid::new_v4().to_string(),
        owner_identity: state.owner_identity,
        channel: ControlChannel::CoreEvents,
        message: ControlMessage::CoreReady,
        authentication: String::new(),
    };
    envelope.authentication = authentication_tag(&core_secret, &envelope);
    let bytes = serde_json::to_vec(&envelope).map_err(|_| ControlError::Malformed)?;
    timeout(CONTROL_IO_TIMEOUT, async {
        stream
            .write_u32(bytes.len() as u32)
            .await
            .map_err(|_| ControlError::Io)?;
        stream
            .write_all(&bytes)
            .await
            .map_err(|_| ControlError::Io)?;
        let size = stream.read_u32().await.map_err(|_| ControlError::Io)? as usize;
        if size == 0 || size > MAX_CONTROL_FRAME_BYTES {
            return Err(ControlError::Malformed);
        }
        let mut response = vec![0u8; size];
        stream
            .read_exact(&mut response)
            .await
            .map_err(|_| ControlError::Io)?;
        let response: ControlResponse =
            serde_json::from_slice(&response).map_err(|_| ControlError::Malformed)?;
        if response.accepted {
            Ok(())
        } else {
            Err(match response.error.as_deref() {
                Some("control_unauthorized") => ControlError::Unauthorized,
                Some("control_stale_generation") => ControlError::StaleGeneration,
                Some("control_replayed") => ControlError::Replayed,
                Some("control_out_of_order") => ControlError::OutOfOrder,
                Some("control_timeout") => ControlError::Timeout,
                _ => ControlError::Io,
            })
        }
    })
    .await
    .map_err(|_| ControlError::Timeout)??;
    let (sender, receiver) = mpsc::channel(4);
    let task = tokio::spawn(async move {
        while let Ok(message) = read_command(&mut stream).await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });
    Ok(CoreCommandReceiver {
        receiver,
        task,
        session_id: state.supervisor_session_id,
        generation: state.child_generation,
    })
}

pub async fn notify_from_env(message: ControlMessage) -> Result<(), ControlError> {
    let state = state_from_env()?;
    if !matches!(
        message,
        ControlMessage::CoreReady
            | ControlMessage::CoreStopping
            | ControlMessage::DesktopLeaseInstalled { .. }
    ) {
        return Err(ControlError::Unauthorized);
    }
    send_control_with_key(&state, message, ControlChannel::CoreEvents, None).await
}

pub fn decode_secret(secret: &str) -> Result<Vec<u8>, ControlError> {
    if secret.is_empty() || secret.len() > 256 {
        return Err(ControlError::Malformed);
    }
    if secret.len().is_multiple_of(2) && secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return (0..secret.len())
            .step_by(2)
            .map(|index| {
                u8::from_str_radix(&secret[index..index + 2], 16)
                    .map_err(|_| ControlError::Malformed)
            })
            .collect();
    }
    Ok(secret.as_bytes().to_vec())
}

pub fn encode_secret(secret: &[u8]) -> String {
    secret.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn new_secret() -> Vec<u8> {
    let mut secret = Vec::with_capacity(32);
    secret.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    secret.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    secret
}

impl From<io::Error> for ControlError {
    fn from(_: io::Error) -> Self {
        Self::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stale_cleanup_cannot_clear_replacement_connection() {
        let core_commands = Arc::new(Mutex::new(None));
        let (old_sender, _old_receiver) = mpsc::channel::<ControlMessage>(1);
        let (new_sender, mut new_receiver) = mpsc::channel(1);
        *core_commands.lock().await = Some(CoreCommandConnection {
            id: 2,
            generation: 2,
            sender: new_sender,
        });

        clear_core_commands(&core_commands, 1, 1).await;
        let current_sender = core_commands
            .lock()
            .await
            .as_ref()
            .expect("replacement connection")
            .sender
            .clone();
        current_sender
            .send(ControlMessage::ShutdownRequested)
            .await
            .expect("replacement remains active");
        assert_eq!(
            new_receiver.recv().await,
            Some(ControlMessage::ShutdownRequested)
        );
        drop(old_sender);
    }
}
