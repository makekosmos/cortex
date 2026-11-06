use super::*;

pub(super) struct ConnectionLoopArgs {
    pub(super) sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
    pub(super) stream: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    >,
    pub(super) command_bus: Arc<CommandBus>,
    pub(super) dictation_host: Arc<DictationHost>,
    pub(super) ark_host: Arc<ArkHost>,
    pub(super) correlation_id: Arc<String>,
    pub(super) client_id: ClientId,
    pub(super) desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    pub(super) shutdown: WsShutdownHandle,
    pub(super) dispatcher: crate::engine_dispatch::EngineDispatcher,
    pub(super) hello: HelloMessage,
}
