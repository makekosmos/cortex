use std::num::NonZeroUsize;
use std::sync::Arc;

use ark_application::sync_storage::SyncRuntimeStorage;

use super::{
    ConnectedServerSessionHost, DirectServerSessionEnd, ResponderHelloRequest,
    ServerSessionActivation, ServerSessionCreateError, ServerSessionHandle,
};

/// Owns the protocol actor for one connected server-side session.
pub struct ConnectedServerSessionDriver {
    actor: super::ServerSessionActor,
}

impl ConnectedServerSessionDriver {
    pub fn channel(
        intent_capacity: NonZeroUsize,
        request: ResponderHelloRequest,
        storage: Arc<dyn SyncRuntimeStorage>,
        host: Arc<dyn ConnectedServerSessionHost>,
    ) -> Result<(ServerSessionHandle, Self, ServerSessionActivation), ServerSessionCreateError>
    {
        let (handle, mut actor, _lease, activation) =
            ServerSessionHandle::channel(intent_capacity)?;
        actor.configure_connected(super::connected::ConnectedServerSession::new(
            request, storage, host,
        ));
        Ok((handle, Self { actor }, activation))
    }

    pub async fn run_controlled(
        &mut self,
        session_io: &mut ark_sync_contracts::session_io::SessionIo,
    ) -> DirectServerSessionEnd {
        super::runner::run_connected_server_session_controlled(session_io, &mut self.actor).await
    }
}
