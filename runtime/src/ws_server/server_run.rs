use super::connection::handle_connection;
use super::types::{WsConnectionResources, WsConnectionSlot};
use super::*;

impl WsServer {
    pub async fn run(self) -> Result<(), WsServerError> {
        let shutdown = self.shutdown_handle();
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                accepted = self.listener.accept() => {
                    // A burst can leave aborted entries in the accept queue
                    // (BSD/macOS report them, Linux does not) — a dropped
                    // socket must not kill the whole listener.
                    let Ok((stream, _peer)) = accepted else {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        continue;
                    };
                    let permit = match self.lifecycle.capacity.clone().try_acquire_owned() {
                        Ok(permit) => permit,
                        Err(_) => {
                            let mut stream = stream;
                            let _ = stream.shutdown().await;
                            continue;
                        }
                    };
                    let _admission = shutdown.lifecycle.admission.lock().unwrap_or_else(
                        |p| p.into_inner()
                    );
                    if self.lifecycle.closed.load(Ordering::Acquire) {
                        drop(permit);
                        drop(_admission);
                        let mut stream = stream;
                        let _ = stream.shutdown().await;
                        continue;
                    }
                    let client_id = match self.dispatcher.allocate_owner() {
                        Ok(owner) => owner,
                        Err(error) => {
                            eprintln!("[mundus.ws] owner allocation failed: {error}");
                            drop(permit);
                            continue;
                        }
                    };
                    let task_id = self.lifecycle.next_task.fetch_add(1, Ordering::Relaxed);
                    let ark_host = self.ark_host.clone();
                    let token = self.auth_token.clone();
                    let bus = self.command_bus.clone();
                    let pomo = self.pomodoro_host.clone();
                    let dict = self.dictation_host.clone();
                    let agent_events = self.agent_events.clone();
                    let protocol_usage = self.protocol_usage.clone();
                    let correlation_id = self.correlation_id.clone();
                    let dispatcher = self.dispatcher.clone();
                    let desktop_authority = self.desktop_authority.clone();
                    let snapshots = self.snapshots.clone();
                    let grants = self.grants.clone();
                    let task_shutdown = shutdown.clone();
                    let resources = Arc::new(Mutex::new(Some(WsConnectionResources {
                        stream: Some(stream),
                        permit: Some(permit),
                        owner_lease: Some(client_id),
                    })));
                    let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
                    self.lifecycle
                        .tasks
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(
                            task_id,
                            WsConnectionSlot::Reserved {
                                resources: resources.clone(),
                                start: start_sender,
                            },
                        );
                    let task_resources = resources.clone();
                    let task = tokio::spawn(async move {
                        if start_receiver.await.is_err() {
                            return;
                        }
                        let Some(mut resources) = task_resources
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .take()
                        else {
                            return;
                        };
                        let Some(stream) = resources.stream.take() else {
                            return;
                        };
                        let Some(owner_lease) = resources.owner_lease.take() else {
                            return;
                        };
                        let _permit = resources.permit.take();
                        // The task cannot execute any connection code until its
                        // JoinHandle has replaced the Reserved slot below.
                        if let Err(e) = handle_connection(
                            stream, ark_host, token, bus, pomo, dict, agent_events,
                            protocol_usage, correlation_id, dispatcher, owner_lease,
                            desktop_authority, snapshots, grants, task_shutdown,
                        ).await {
                            eprintln!("[mundus.ws] connection error: {e}");
                        }
                    });
                    let old = self
                        .lifecycle
                        .tasks
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .insert(task_id, WsConnectionSlot::Installed(task));
                    debug_assert!(matches!(old, Some(WsConnectionSlot::Reserved { .. })));
                    if let Some(WsConnectionSlot::Reserved { start, .. }) = old {
                        let _ = start.send(());
                    }
                }
            }
            shutdown.reap().await;
        }
        shutdown.reap().await;
        Ok(())
    }
}
