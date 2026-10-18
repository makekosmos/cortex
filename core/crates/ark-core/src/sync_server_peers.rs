use super::*;

#[path = "sync_server_peers/committed_feed.rs"]
mod committed_feed;
pub(super) use committed_feed::{
    handle_committed_feed_ack, handle_committed_feed_receipt_ack, pump_committed_feed,
};
#[path = "sync_server_peers/task_merge.rs"]
mod task_merge;
pub(super) use task_merge::{handle_task_merge_batch_ack, pump_task_merge_batch};

impl SyncServer {
    pub async fn broadcast_task_merge_batches(&self) {
        let peer_ids = {
            let peers = self.peers.lock().await;
            peers
                .iter()
                .filter(|(_, peer)| {
                    peer.ready_peer.is_some() && peer.outbound_snapshot.has_been_acknowledged()
                })
                .map(|(peer_id, _)| *peer_id)
                .collect::<Vec<_>>()
        };
        for peer_id in peer_ids {
            if let Err(error) = pump_task_merge_batch(&self.storage, &self.peers, peer_id).await {
                eprintln!("{TAG} task merge broadcast failed: {error}");
            }
        }
    }

    /// Stop the server.
    pub async fn stop(&self) {
        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        // Join the accept/ticker task: until it exits it holds a clone of
        // `storage`, keeping the test db connection open past teardown.
        if let Some(task) = self.accept_task.lock().await.take() {
            if let Err(error) = task.await {
                eprintln!("{TAG} accept task ended abnormally: {error}");
            }
        }
        // The accept task has ended, so no new connection can be recorded.
        // Each connection's reader holds `storage` and may be partway through
        // a db write. Ask the peers to close and drop the writers' senders,
        // then give the connection tasks a bounded chance to finish on their
        // own: aborting a reader mid-write would leave its blocking db call
        // running past the join (KOS-270). Whatever is still alive after the
        // bound is aborted.
        {
            let mut peers = self.peers.lock().await;
            for peer in peers.values() {
                peer.session.revoke();
                peer.tx.close();
            }
            peers.clear();
        }
        let tasks: Vec<tokio::task::JoinHandle<()>> = self
            .conn_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        join_connection_tasks(tasks, Duration::from_secs(2)).await;
        // An aborted handler drops SessionIo, which signals its transport
        // supervisor to close. Keep and join those handles separately.
        let transport_tasks: Vec<tokio::task::JoinHandle<()>> = self
            .transport_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        join_connection_tasks(transport_tasks, Duration::from_secs(2)).await;
        eprintln!("{TAG} Server stopped");
    }

    /// Broadcast a live change to all authenticated peers.
    pub async fn broadcast_live_change(&self, exclude_device_id: Option<&str>) {
        let peer_ids = {
            let peers = self.peers.lock().await;
            let mut ids = Vec::new();
            for (peer_id, peer) in peers.iter() {
                if peer.ready_peer.is_none() {
                    continue;
                }
                if let Some(exclude) = exclude_device_id {
                    if peer.device_id == exclude {
                        continue;
                    }
                }
                if !peer.outbound_snapshot.has_been_acknowledged() {
                    continue;
                }
                ids.push(*peer_id);
            }
            ids
        };
        for peer_id in peer_ids {
            if let Err(error) = pump_committed_feed(&self.storage, &self.peers, peer_id).await {
                eprintln!("{TAG} committed feed wake failed: {error}");
            }
        }
    }

    /// Update the version vector for a locally-mutated entity.
    pub async fn update_entity_hlc(&self, entity_id: &str) -> Result<String, String> {
        let device_id = self.device_id.read().await.clone();
        let hlc_str = HLC::now(&device_id).to_string();
        let mut vector = load_version_vector(&self.storage).await?;
        vector.insert(entity_id.to_string(), hlc_str.clone());
        save_version_vector(&self.storage, &vector).await?;
        Ok(hlc_str)
    }

    pub(super) async fn load_known_peers(&self) -> Result<(), String> {
        if let Some(peers) = self.storage.load_known_peers().await? {
            let removed = load_removed_peer_ids(&self.storage).await?;
            *self.known_peer_records.lock().await = peers
                .into_iter()
                .filter(|peer| !removed.iter().any(|id| id == &peer.device_id))
                .collect();
        }
        Ok(())
    }

    /// Purge stale self-referencing peer records. A record is "self" if:
    ///   - its device_id matches our current device_id, OR
    ///   - every address in its list is one of our own addresses (prior-run
    ///     phantom with a different device_id).
    ///
    /// Called on start after loading known peers. Prevents self-connect loops
    /// where stale records from dev iterations fire SyncClients that connect
    /// to our own SyncServer.
    pub async fn purge_self_peers(&self) -> Result<usize, String> {
        let my_device_id = self.device_id.read().await.clone();
        let my_addresses = self.own_addresses.read().await.clone();
        let mut known = self.known_peer_records.lock().await;
        let before = known.len();
        known.retain(|p| {
            if p.device_id == my_device_id {
                return false;
            }
            if p.addresses.is_empty() {
                return true;
            }
            let all_ours = p.addresses.iter().all(|a| my_addresses.contains(a));
            !all_ours
        });
        let removed = before - known.len();
        if removed > 0 {
            eprintln!("{TAG} Purged {removed} stale self-peer records");
            save_known_peers(&self.storage, &known).await?;
        }
        Ok(removed)
    }
}

async fn join_connection_tasks(tasks: Vec<tokio::task::JoinHandle<()>>, grace_period: Duration) {
    let deadline = tokio::time::Instant::now() + grace_period;
    while tasks.iter().any(|task| !task.is_finished()) {
        if tokio::time::Instant::now() >= deadline {
            for task in &tasks {
                if !task.is_finished() {
                    task.abort();
                }
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    for task in tasks {
        if let Err(error) = task.await {
            if !error.is_cancelled() {
                eprintln!("{TAG} connection task ended abnormally: {error}");
            }
        }
    }
}

#[cfg(test)]
mod connection_task_tests {
    use super::join_connection_tasks;
    use std::time::Duration;
    use tokio::sync::oneshot;

    struct DropNotice(Option<oneshot::Sender<()>>);

    impl Drop for DropNotice {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[tokio::test]
    async fn timed_out_connection_tasks_are_aborted_and_joined() {
        let (started_tx, started_rx) = oneshot::channel();
        let (dropped_tx, mut dropped_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _notice = DropNotice(Some(dropped_tx));
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.expect("connection task started");

        join_connection_tasks(vec![task], Duration::ZERO).await;
        assert_eq!(
            dropped_rx.try_recv(),
            Ok(()),
            "aborted task is dropped before shutdown returns"
        );
    }
}
