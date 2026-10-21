    {
        let Some(admission) = pending_registry_admission.as_mut() else {
            eprintln!("{TAG} registry control arrived before Hello");
            break;
        };
        let became_ready = match admission
            .handle(
                sync_msg,
                registry_admission::Context {
                    local_device_id: &device_id,
                    storage: &storage,
                    tx: &tx,
                    authenticated_tx: &authenticated_tx,
                    peer: &peer,
                    on_registry_events: &on_registry_events,
                    on_connected: &on_connected,
                },
            )
            .await
        {
            Ok(became_ready) => became_ready,
            Err(error) => {
                if matches!(
                    &error,
                    registry_admission::PendingRegistryAdmissionError::SessionRevoked
                ) {
                    stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                eprintln!("{TAG} direct registry admission failed: {error}");
                break;
            }
        };
        if became_ready && !client_session.mark_ready() {
            eprintln!("{TAG} registry admission completed outside the client session phase");
            break;
        }
        continue;
    }
