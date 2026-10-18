use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub type ClientWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
