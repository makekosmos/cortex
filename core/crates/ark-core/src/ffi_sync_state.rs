use super::*;

impl ArkCore {
    pub(super) fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let outer = self.db.lock().unwrap_or_else(|e| e.into_inner());
        let shared = outer
            .as_ref()
            .cloned()
            .ok_or_else(|| err("Database not opened. Call open_db first."))?;
        drop(outer);
        let inner = shared.lock().unwrap_or_else(|e| e.into_inner());
        f(&inner)
    }

    pub(super) async fn stop_sync_inner(&self) {
        let runtime = {
            let mut guard = self.sync.lock().await;
            guard.take()
        };
        if let Some(runtime) = runtime {
            runtime.beacon.stop().await;
            if let Some(relay) = runtime.relay.as_ref() {
                relay.stop();
            }
            runtime.server.stop().await;
            let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
            for client in clients {
                client.stop();
            }
        }
    }
}
