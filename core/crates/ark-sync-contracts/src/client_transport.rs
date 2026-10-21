pub struct ConnectedClientTransport {
    pub io: crate::session_io::SessionIo,
    pub selected_address: String,
}

#[async_trait::async_trait]
pub trait DirectClientConnector: Send + Sync {
    async fn connect(
        &self,
        addresses: &[String],
    ) -> Result<Option<ConnectedClientTransport>, crate::session_io::SessionIoError>;
}
