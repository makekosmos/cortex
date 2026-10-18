use std::time::Duration;
use std::{io, net::SocketAddr};

use ark_sync::session_io::{SessionIo, SessionIoDriver, SessionIoEvent, SessionOutboundGuard};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

pub type ClientWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type ServerWebSocket = WebSocketStream<TcpStream>;

const SESSION_QUEUE_CAPACITY: usize = 64;

enum SessionTransportFinish {
    Shutdown,
    Writer,
    Reader,
}

pub struct Listener {
    listener: TcpListener,
}

impl Listener {
    pub async fn bind(address: &str) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(address).await?,
        })
    }

    pub async fn accept(&self) -> io::Result<(ServerWebSocket, SocketAddr)> {
        let (stream, peer_address) = self.listener.accept().await?;
        let websocket = accept_async(stream)
            .await
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok((websocket, peer_address))
    }
}

pub fn session_io<S>(
    websocket: WebSocketStream<S>,
) -> Result<SessionIo, ark_sync::session_io::SessionIoError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    session_io_with_guard(websocket, None)
}

pub fn session_io_with_guard<S>(
    websocket: WebSocketStream<S>,
    outbound_guard: Option<std::sync::Arc<dyn SessionOutboundGuard>>,
) -> Result<SessionIo, ark_sync::session_io::SessionIoError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    SessionIo::start_transport_with_guard(SESSION_QUEUE_CAPACITY, outbound_guard, move |driver| {
        tokio::spawn(run_session_transport(websocket, driver))
    })
}

async fn run_session_transport<S>(websocket: WebSocketStream<S>, driver: SessionIoDriver)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let SessionIoDriver {
        incoming,
        outgoing,
        control_outgoing,
        close_after_flush,
        pending_sends,
        close_result,
        worker_completion,
        outbound_guard,
        shutdown,
        stop,
    } = driver;
    let (sink, stream) = websocket.split();
    let (control_tx, control_rx) = mpsc::channel(8);
    let mut writer = Box::pin(write_session_frames(SessionWriter {
        sink,
        outgoing,
        control_outgoing,
        outbound_guard,
        control: control_rx,
        close_after_flush,
        pending_sends,
        close_result,
        incoming: incoming.clone(),
        shutdown: shutdown.clone(),
    }));
    let mut reader = Box::pin(read_session_frames(
        stream,
        incoming.clone(),
        control_tx,
        shutdown.clone(),
    ));

    let mut shutdown = shutdown;
    let finished = tokio::select! {
        _ = shutdown.changed() => SessionTransportFinish::Shutdown,
        _ = &mut writer => SessionTransportFinish::Writer,
        _ = &mut reader => SessionTransportFinish::Reader,
    };
    let _ = stop.send(true);
    match finished {
        SessionTransportFinish::Shutdown => {
            tokio::join!(writer, reader);
        }
        SessionTransportFinish::Writer => reader.await,
        SessionTransportFinish::Reader => writer.await,
    }
    worker_completion.complete();
}

enum WebSocketControl {
    Pong(tokio_tungstenite::tungstenite::Bytes),
}

struct SessionWriter<S> {
    sink: futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    outgoing: mpsc::Receiver<ark_sync::protocol::LanSyncMessage>,
    control_outgoing: mpsc::Receiver<ark_sync::protocol::LanSyncMessage>,
    outbound_guard: Option<std::sync::Arc<dyn SessionOutboundGuard>>,
    control: mpsc::Receiver<WebSocketControl>,
    close_after_flush: mpsc::Receiver<()>,
    pending_sends: ark_sync::session_io::SessionIoPendingSends,
    close_result: watch::Sender<Option<Result<(), String>>>,
    incoming: mpsc::Sender<SessionIoEvent>,
    shutdown: watch::Receiver<bool>,
}

async fn write_session_frames<S>(writer: SessionWriter<S>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let SessionWriter {
        mut sink,
        mut outgoing,
        mut control_outgoing,
        outbound_guard,
        mut control,
        mut close_after_flush,
        pending_sends,
        close_result,
        incoming,
        mut shutdown,
    } = writer;
    let mut outgoing_open = true;
    let mut control_outgoing_open = true;
    let mut close_channel_open = true;
    let mut close_requested = false;
    loop {
        if close_requested
            && outgoing.is_empty()
            && control_outgoing.is_empty()
            && !pending_sends.has_pending()
        {
            break;
        }
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            close = close_after_flush.recv(), if close_channel_open && !close_requested => {
                match close {
                    Some(()) => close_requested = true,
                    None => close_channel_open = false,
                }
            }
            _ = pending_sends.changed(), if close_requested => {}
            outgoing_message = control_outgoing.recv(), if control_outgoing_open => {
                match outgoing_message {
                    Some(message) => {
                        if !write_sync_frame(
                            &mut sink,
                            message,
                            &incoming,
                            outbound_guard.as_deref(),
                        )
                        .await
                        {
                            return;
                        }
                    }
                    None => control_outgoing_open = false,
                }
            }
            outgoing_message = outgoing.recv(), if outgoing_open => {
                match outgoing_message {
                    Some(message) => {
                        if !write_sync_frame(
                            &mut sink,
                            message,
                            &incoming,
                            outbound_guard.as_deref(),
                        )
                        .await
                        {
                            return;
                        }
                    }
                    None => outgoing_open = false,
                }
            }
            control_message = control.recv() => {
                let Some(WebSocketControl::Pong(payload)) = control_message else { break; };
                if let Err(error) = sink.send(Message::Pong(payload)).await {
                    let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
                    return;
                }
            }
        }
        if !outgoing_open && !control_outgoing_open {
            break;
        }
    }
    match tokio::time::timeout(Duration::from_secs(2), sink.close()).await {
        Ok(Ok(())) => {
            let _ = close_result.send(Some(Ok(())));
        }
        Ok(Err(error)) => {
            let _ = close_result.send(Some(Err(error.to_string())));
            let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
        }
        Err(_) => {
            let _ = close_result.send(Some(Err("websocket close after flush timed out".into())));
            let _ = incoming.try_send(SessionIoEvent::Failed(
                "websocket close after flush timed out".into(),
            ));
        }
    }
}

async fn write_sync_frame<S>(
    sink: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    message: ark_sync::protocol::LanSyncMessage,
    incoming: &mpsc::Sender<SessionIoEvent>,
    outbound_guard: Option<&dyn SessionOutboundGuard>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if let Some(guard) = outbound_guard {
        if !guard.allow(&message).await {
            return true;
        }
    }
    let text = match serde_json::to_string(&message) {
        Ok(text) => text,
        Err(error) => {
            let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
            return false;
        }
    };
    if let Err(error) = sink.send(Message::Text(text.into())).await {
        let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
        return false;
    }
    true
}

async fn read_session_frames<S>(
    mut stream: futures_util::stream::SplitStream<WebSocketStream<S>>,
    incoming: mpsc::Sender<SessionIoEvent>,
    control: mpsc::Sender<WebSocketControl>,
    mut shutdown: watch::Receiver<bool>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            incoming_message = stream.next() => {
                match incoming_message {
                    Some(Ok(Message::Text(text))) => {
                        let event = serde_json::from_str(text.as_ref())
                            .map(|message| SessionIoEvent::Message(Box::new(message)))
                            .unwrap_or(SessionIoEvent::MalformedMessage);
                        if !publish_session_event(&incoming, event, &mut shutdown).await {
                            return;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        tokio::select! {
                            _ = shutdown.changed() => return,
                            result = control.send(WebSocketControl::Pong(payload)) => {
                                if result.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => {
                        let _ = publish_session_event(
                            &incoming,
                            SessionIoEvent::Closed,
                            &mut shutdown,
                        ).await;
                        return;
                    }
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Frame(_))) => {
                        if !publish_session_event(
                            &incoming,
                            SessionIoEvent::MalformedMessage,
                            &mut shutdown,
                        ).await {
                            return;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
                        return;
                    }
                }
            }
        }
    }
}

async fn publish_session_event(
    incoming: &mpsc::Sender<SessionIoEvent>,
    event: SessionIoEvent,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        _ = shutdown.changed() => false,
        result = incoming.send(event) => result.is_ok(),
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn race_connect(addresses: &[String]) -> Option<(ClientWebSocket, String)> {
    if addresses.is_empty() {
        return None;
    }

    let (result_tx, result_rx) = oneshot::channel();
    let result_tx = std::sync::Arc::new(Mutex::new(Some(result_tx)));
    let mut handles = Vec::new();

    for address in addresses {
        let url = format!("ws://{address}");
        let address = address.clone();
        let result_tx = result_tx.clone();

        handles.push(tokio::spawn(async move {
            if let Ok(Ok((stream, _))) =
                tokio::time::timeout(CONNECT_TIMEOUT, connect_async(&url)).await
            {
                let mut sender = result_tx.lock().await;
                if let Some(sender) = sender.take() {
                    let _ = sender.send((stream, address));
                }
            }
        }));
    }

    match result_rx.await {
        Ok(result) => {
            for handle in handles {
                handle.abort();
            }
            Some(result)
        }
        Err(_) => None,
    }
}

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod tests;
