use super::*;

pub(super) async fn handle(state: &Arc<ServiceState>, request: Request) -> Result<Value, String> {
    match request {
        Request::GetSyncKv { key } => with_conn(state, |conn| {
            let value = db::get_sync_kv(conn, &key)?;
            Ok(json!(value))
        }),

        Request::SetSyncKv { key, value } => with_conn(state, |conn| {
            db::set_sync_kv(conn, &key, &value)?;
            Ok(json!(true))
        }),

        Request::ExternalRefsUpsert {
            connector_id,
            account_id,
            external_type,
            external_id,
            object_id,
            revision,
            hash,
            state: ref_state,
        } => with_write_tx(state, |conn| {
            crate::data_platform::ensure_schema(conn)?;
            crate::data_platform::upsert_external_ref(
                conn,
                &connector_id,
                &account_id,
                &external_type,
                &external_id,
                &object_id,
                revision.as_deref(),
                hash.as_deref(),
                &ref_state,
            )?;
            Ok(json!(true))
        }),

        Request::ClearAll => with_conn(state, |conn| {
            db::clear_all(conn)?;
            Ok(json!(null))
        }),

        Request::DeleteTrashed => with_conn(state, |conn| {
            let count = db::delete_trashed(conn)?;
            Ok(json!(count))
        }),

        Request::DbBackup { dest_path } => {
            let src_path = current_db_path(state)?;
            let pages = backup_pages_per_step();
            let pause = std::time::Duration::from_millis(backup_pause_ms());
            let dest_for_thread = dest_path.clone();
            let state_for_thread = Arc::clone(state);
            // Fire-and-forget: копирование идёт на отдельном background-priority
            // потоке через ОТДЕЛЬНЫЙ read-коннекшн (не держит глобальный DB
            // mutex) — серийный RPC-loop сразу свободен для других запросов.
            // Завершение сообщается событием `db_backup_result`; kepler-backend
            // ждёт его, чтобы записать last_backup_ts + ротацию.
            // KOS-51: BACKUP_GATE удерживается на всё копирование —
            // `db_backup_restore` не может начаться посреди backup'а.
            std::thread::Builder::new()
                .name("ark-db-backup".to_string())
                .spawn(move || {
                    let _bg = background_priority::BackgroundThreadGuard::enter();
                    let _gate = state_for_thread
                        .backup_gate
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let result =
                        db::backup_to_file_chunked(&src_path, &dest_for_thread, pages, pause);
                    let event = match &result {
                        Ok(()) => json!({
                            "event": "db_backup_result",
                            "ok": true,
                            "dest": dest_for_thread,
                        }),
                        Err(e) => json!({
                            "event": "db_backup_result",
                            "ok": false,
                            "dest": dest_for_thread,
                            "error": e,
                        }),
                    };
                    emit_event(event);
                })
                .map_err(|e| format!("spawn db_backup thread failed: {e}"))?;
            Ok(json!({ "started": true, "dest": dest_path }))
        }

        // KOS-51: list/validate — read-only над backups dir + live conn,
        // BACKUP_GATE не нужен (не мутируют ничего).
        Request::DbBackupList => {
            let db_path = current_db_path(state)?;
            let backups = db::list_snapshots(&db_path)?;
            Ok(json!({ "backups": backups }))
        }

        Request::DbBackupValidate { backup_id } => {
            let db_path = current_db_path(state)?;
            with_conn(state, |conn| {
                let validation = db::validate_snapshot(conn, &db_path, &backup_id)?;
                serde_json::to_value(validation).map_err(|e| e.to_string())
            })
        }

        // KOS-51: атомарный restore. Порядок захвата: BACKUP_GATE → DB mutex
        // (with_conn_mut). Restore держит оба на всю операцию — ни backup, ни
        // обычные ARK ops не выполняются параллельно, момента с отсутствующей
        // или частично заменённой primary DB нет (Online Backup API пишет
        // destination транзакционно).
        Request::DbBackupRestore { backup_id } => {
            let db_path = current_db_path(state)?;
            let _gate = state.backup_gate.lock().unwrap_or_else(|e| e.into_inner());
            with_conn_mut(state, |conn| {
                let report = db::restore_snapshot(conn, &db_path, &backup_id)?;
                serde_json::to_value(report).map_err(|e| e.to_string())
            })
        }

        Request::StartSync {
            space_id,
            device_id,
            device_name,
            port,
            seed_addresses,
            relay_url,
            relay_api_key,
            auth_secret,
            use_iroh,
            iroh_peer_ticket,
            discovery_enabled,
        } => {
            handle_start_sync(
                state,
                space_id,
                device_id,
                device_name,
                port,
                seed_addresses,
                relay_url,
                relay_api_key,
                auth_secret,
                use_iroh,
                iroh_peer_ticket,
                discovery_enabled,
            )
            .await
        }

        Request::StopSync => {
            handle_stop_sync(state).await;
            Ok(json!(true))
        }

        Request::BroadcastChange { entity } => handle_broadcast_change(state, entity).await,

        Request::GetConnectedPeers => handle_get_connected_peers(state).await,

        Request::GetSyncSnapshot => handle_get_sync_snapshot(state).await,

        Request::DisconnectPeer { device_id } => handle_disconnect_peer(state, device_id).await,

        Request::ConnectWithPairingCode { pairing_code } => {
            handle_connect_with_pairing_code(state, pairing_code).await
        }

        Request::LeaveSpace => {
            handle_stop_sync(state).await;
            // Also purge any persisted self-reference peer records so the
            // next start_sync on this or any other space does not inherit
            // phantom records. The underlying SyncServer did this inside its
            // own `start()` but we may be leaving without restarting.
            Ok(json!(true))
        }

        Request::AddSeedPeer { addresses } => handle_add_seed_peer(state, addresses).await,

        Request::GetOwnAddresses { port } => {
            let port = port.unwrap_or(LAN_SYNC_PORT);
            Ok(json!(get_own_addresses(port)))
        }

        Request::GetHostDeviceName => Ok(json!(get_host_device_name())),

        Request::GetOwnIrohTicket => handle_get_own_iroh_ticket(state).await,
        _ => unreachable!("request routed to the wrong runtime handler"),
    }
}
