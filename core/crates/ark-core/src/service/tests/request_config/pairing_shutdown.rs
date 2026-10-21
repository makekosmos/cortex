use super::*;

#[tokio::test]
async fn show_pairing_code_params_escalate_loopback_to_lan() {
    // The loopback fixture cannot safely exercise an all-interfaces bind, so
    // verify the parameters selected for explicit LAN pairing directly.
    let (runtime, _database) = loopback_runtime_fixture().await;
    let params = crate::service::sync::lan_opt_in_params(&runtime, None);
    assert_eq!(params.bind, SyncBind::AllInterfaces);
    assert!(params.use_iroh);
    assert!(params.iroh_peer_ticket.is_none());
    assert_eq!(params.space_id, "space-a");
    assert_eq!(params.device_id, "device-a");
    runtime
        .storage
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_pairing_code_returns_existing_ticket_without_restart() {
    // A LAN-ready runtime must not restart: the request answers with the
    // ticket it already has.
    let state = test_state();
    let (mut runtime, _database) = loopback_runtime_fixture().await;
    runtime.start_params.bind = SyncBind::AllInterfaces;
    runtime.start_params.use_iroh = true;
    runtime.transport_choice = Some(TransportChoice::Iroh);
    runtime.iroh_our_ticket = watch::channel(Some("endpoint-our-ticket".to_string())).1;
    *state.sync.lock().await = Some(Arc::new(runtime));

    let result = handle_request(&state, Request::ShowPairingCode)
        .await
        .expect("show_pairing_code should return the running ticket");
    assert_eq!(result.as_str(), Some("endpoint-our-ticket"));
    // Bind untouched — still the all-interfaces runtime we installed.
    let after = state.sync.lock().await;
    assert_eq!(
        after.as_ref().unwrap().start_params.bind,
        SyncBind::AllInterfaces
    );
    drop(after);
    let runtime = state.sync.lock().await.take().unwrap();
    runtime
        .storage
        .shutdown_for_test(std::time::Instant::now() + std::time::Duration::from_secs(5))
        .await
        .unwrap();
}
