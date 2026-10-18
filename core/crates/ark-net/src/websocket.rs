use std::time::Duration;
use std::{io, net::SocketAddr};

use ark_sync::session_io::{SessionIo, SessionIoDriver, SessionIoEvent};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio::task::JoinSet;
use tokio_tungstenite::{
    accept_async, connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};

pub type ClientWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type ServerWebSocket = WebSocketStream<TcpStream>;

const SESSION_QUEUE_CAPACITY: usize = 64;

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
    SessionIo::start_transport(SESSION_QUEUE_CAPACITY, move |driver| {
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
        shutdown,
        stop,
    } = driver;
    let (sink, stream) = websocket.split();
    let (control_tx, control_rx) = mpsc::channel(8);
    let mut tasks = JoinSet::new();
    tasks.spawn(write_session_frames(
        sink,
        outgoing,
        control_outgoing,
        control_rx,
        incoming.clone(),
        shutdown.clone(),
    ));
    tasks.spawn(read_session_frames(
        stream,
        incoming.clone(),
        control_tx,
        shutdown.clone(),
    ));

    let mut shutdown = shutdown;
    tokio::select! {
        _ = shutdown.changed() => {}
        result = tasks.join_next() => {
            if let Some(Err(error)) = result {
                let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
            }
        }
    }
    let _ = stop.send(true);
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                let _ = incoming.try_send(SessionIoEvent::Failed(error.to_string()));
            }
        }
    }
}

enum WebSocketControl {
    Pong(tokio_tungstenite::tungstenite::Bytes),
}

async fn write_session_frames<S>(
    mut sink: futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    mut outgoing: mpsc::Receiver<ark_sync::protocol::LanSyncMessage>,
    mut control_outgoing: mpsc::Receiver<ark_sync::protocol::LanSyncMessage>,
    mut control: mpsc::Receiver<WebSocketControl>,
    incoming: mpsc::Sender<SessionIoEvent>,
    mut shutdown: watch::Receiver<bool>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut outgoing_open = true;
    let mut control_outgoing_open = true;
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            outgoing_message = control_outgoing.recv(), if control_outgoing_open => {
                match outgoing_message {
                    Some(message) => {
                        if !write_sync_frame(&mut sink, message, &incoming).await {
                            return;
                        }
                    }
                    None => control_outgoing_open = false,
                }
            }
            outgoing_message = outgoing.recv(), if outgoing_open => {
                match outgoing_message {
                    Some(message) => {
                        if !write_sync_frame(&mut sink, message, &incoming).await {
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
    let _ = tokio::time::timeout(Duration::from_secs(2), sink.close()).await;
}

async fn write_sync_frame<S>(
    sink: &mut futures_util::stream::SplitSink<WebSocketStream<S>, Message>,
    message: ark_sync::protocol::LanSyncMessage,
    incoming: &mpsc::Sender<SessionIoEvent>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
mod listener_tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    #[tokio::test]
    async fn listener_accepts_a_websocket_and_receives_a_text_frame() {
        let listener = Listener::bind("127.0.0.1:0").await.expect("listener binds");
        let address = listener
            .listener
            .local_addr()
            .expect("listener address is available");
        let client = tokio::spawn(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}"))
                .await
                .expect("client handshake succeeds");
            socket
                .send(Message::Text("hello".into()))
                .await
                .expect("client sends text");
        });
        let (mut socket, peer_address) = listener.accept().await.expect("server accepts client");
        assert!(peer_address.ip().is_loopback());
        assert!(matches!(
            socket.next().await,
            Some(Ok(Message::Text(text))) if text == "hello"
        ));
        client.await.expect("client task completes");
    }

    #[tokio::test]
    async fn session_io_translates_frames_and_joins_after_close() {
        let listener = Listener::bind("127.0.0.1:0").await.expect("listener binds");
        let address = listener
            .listener
            .local_addr()
            .expect("listener address is available");
        let client = tokio::spawn(async move {
            let (mut socket, _) = connect_async(format!("ws://{address}"))
                .await
                .expect("client connects");
            socket
                .send(Message::Text(
                    serde_json::to_string(&ark_sync::protocol::LanSyncMessage::Ping { ts: 42 })
                        .expect("ping serializes")
                        .into(),
                ))
                .await
                .expect("client sends ping");
            let Some(Ok(Message::Text(text))) = socket.next().await else {
                panic!("client receives server protocol message");
            };
            assert!(matches!(
                serde_json::from_str::<ark_sync::protocol::LanSyncMessage>(&text)
                    .expect("server message deserializes"),
                ark_sync::protocol::LanSyncMessage::Pong { ts: 42 }
            ));
            socket
                .send(Message::Text("malformed".into()))
                .await
                .expect("client sends malformed protocol message");
            socket.close(None).await.expect("client closes websocket");
        });

        let (websocket, _) = listener.accept().await.expect("server accepts client");
        let mut session = session_io(websocket).expect("session transport starts");
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::Message(message))
                if matches!(*message, ark_sync::protocol::LanSyncMessage::Ping { ts: 42 })
        ));
        session
            .send(ark_sync::protocol::LanSyncMessage::Pong { ts: 42 })
            .await
            .expect("session sends protocol message");
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::MalformedMessage)
        ));
        assert!(matches!(
            session.receive().await,
            Some(SessionIoEvent::Closed)
        ));
        client.await.expect("client completes");
        session.finish().await.expect("transport worker joins");
    }
}

#[cfg(test)]
mod tests {
    use super::race_connect;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[tokio::test]
    async fn connects_to_the_first_available_websocket_address() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener binds");
        let address = listener
            .local_addr()
            .expect("test listener address loads")
            .to_string();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("client connects");
            accept_async(stream)
                .await
                .expect("websocket handshake succeeds")
        });

        let connected = race_connect(&["127.0.0.1:0".into(), address.clone()])
            .await
            .expect("available websocket address connects");
        assert_eq!(connected.1, address);
        drop(connected);
        drop(server.await.expect("websocket server finishes"));
    }
}
