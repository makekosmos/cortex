use std::time::Duration;
use std::{io, net::SocketAddr};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::{accept_async, connect_async, MaybeTlsStream, WebSocketStream};

pub type ClientWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type ServerWebSocket = WebSocketStream<TcpStream>;

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
