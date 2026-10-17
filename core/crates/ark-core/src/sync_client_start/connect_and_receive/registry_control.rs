    {
        match direct_ws_control::handle(
            &sync_msg,
            &tx,
            authenticated,
            &on_peer_list,
            &device_id,
            &peer_device_id_actual,
        )
        .await
        {
            direct_ws_control::DirectWsControl::Handled => continue,
            direct_ws_control::DirectWsControl::CloseConnection => break,
            direct_ws_control::DirectWsControl::NotControl => {}
        }
        if matches!(
            &sync_msg,
            LanSyncMessage::RegistryAdmissionStart { .. }
                | LanSyncMessage::RegistryAdmissionProof { .. }
                | LanSyncMessage::RegistryAdmissionConfirm { .. }
                | LanSyncMessage::RegistrySyncStart { .. }
                | LanSyncMessage::RegistryEventsPage { .. }
                | LanSyncMessage::RegistrySyncReady { .. }
                | LanSyncMessage::RegistryEventUpdate { .. }
        ) {
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
                    if error == "registry_admission_session_revoked" {
                        stopped.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    eprintln!("{TAG} direct registry admission failed: {error}");
                    break;
                }
            };
            if became_ready {
                authenticated = true;
            }
            continue;
        }
    }
