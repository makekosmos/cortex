use super::super::{
    send_msg, AuthenticatedTx, ClientFeedInFlight, ClientMessageSender, ClientOnChangeCallback,
    ClientOnConnectedCallback, ClientOnDisconnectedCallback, ClientOnPeerListCallback,
    ClientOnRegistryEventsCallback, RECONNECT_BASE_MS, TAG,
};
use super::{
    feed_recovery, live_messages, registry_admission, registry_control, server_hello, snapshot_ack,
    snapshot_drive, sync_changes, sync_policy,
};
use domain::sync_contracts::client_transport::ConnectedClientTransport;
use domain::sync_contracts::wire::LanSyncMessage;
use domain::sync_storage::SyncRuntimeStorage;
use domain::types::PeerRecord;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

pub(super) struct Context<'a> {
    pub(super) storage: &'a Arc<dyn SyncRuntimeStorage>,
    pub(super) peer: &'a Arc<RwLock<PeerRecord>>,
    pub(super) peer_record: &'a PeerRecord,
    pub(super) device_id: &'a String,
    pub(super) device_name: &'a String,
    pub(super) space_id: &'a String,
    pub(super) own_addresses: &'a [String],
    pub(super) auth_secret: Option<&'a str>,
    pub(super) task_merge_bootstrap_v2: bool,
    pub(super) platform: &'a Option<String>,
    pub(super) app_version: &'a Option<String>,
    pub(super) stopped: &'a Arc<AtomicBool>,
    pub(super) authenticated_tx: &'a AuthenticatedTx,
    pub(super) feed_in_flight: &'a ClientFeedInFlight,
    pub(super) on_change: &'a Arc<Mutex<Option<ClientOnChangeCallback>>>,
    pub(super) on_connected: &'a Arc<Mutex<Option<ClientOnConnectedCallback>>>,
    pub(super) on_disconnected: &'a Arc<Mutex<Option<ClientOnDisconnectedCallback>>>,
    pub(super) on_peer_list: &'a Arc<Mutex<Option<ClientOnPeerListCallback>>>,
    pub(super) on_registry_events: &'a Arc<Mutex<Option<ClientOnRegistryEventsCallback>>>,
}

pub(super) enum Disposition {
    RetryImmediately,
    Backoff,
    Stop,
}

pub(super) async fn run<'a>(
    result: Result<
        Option<ConnectedClientTransport>,
        domain::sync_contracts::session_io::SessionIoError,
    >,
    context: Context<'a>,
    reconnect_delay: &'a mut u64,
) -> Disposition {
    let Context {
        storage,
        peer,
        peer_record,
        device_id,
        device_name,
        space_id,
        own_addresses,
        auth_secret,
        task_merge_bootstrap_v2,
        platform,
        app_version,
        stopped,
        authenticated_tx,
        feed_in_flight,
        on_change,
        on_connected,
        on_disconnected,
        on_peer_list,
        on_registry_events,
    } = context;
    match result {
        Ok(Some(transport)) => {
            let domain::sync_contracts::client_transport::ConnectedClientTransport {
                io: mut session_io,
                selected_address: winning_addr,
            } = transport;
            eprintln!(
                "{TAG} Connected to {} via {winning_addr}",
                peer_record.device_name
            );

            {
                let mut p = peer.write().await;
                p.last_address = Some(winning_addr.clone());
            }

            *reconnect_delay = RECONNECT_BASE_MS;
            *feed_in_flight.lock().await = None;

            let client_hello = crate::client_session::build_client_hello(
                crate::client_session::ClientHelloRequest {
                    device_id,
                    device_name,
                    space_id,
                    addresses: own_addresses,
                    auth_secret,
                    task_merge_bootstrap_v2,
                    platform: platform.clone(),
                    app_version: app_version.clone(),
                },
            );
            let tx = ClientMessageSender::new(
                session_io.sender(),
                Arc::clone(storage),
                Arc::clone(peer),
            );
            if tx.send(client_hello.message).await.is_err() {
                let _ = session_io.finish().await;
                return Disposition::RetryImmediately;
            }

            let mut client_session = crate::client_session::DirectClientSession::new();
            let mut pending_registry_admission: Option<
                registry_admission::PendingRegistryAdmission,
            > = None;
            let mut feed_recovery = tokio::time::interval(Duration::from_secs(1));
            feed_recovery.tick().await;
            let mut snapshot_drive = None;
            let mut pending_remote_vector = None;

            loop {
                tokio::select! {
                    result = async {
                        match snapshot_drive.as_mut() {
                            Some(task) => task.await,
                            None => std::future::pending().await,
                        }
                    } => {
                        snapshot_drive = None;
                        match result {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => {
                                eprintln!("{TAG} outbound snapshot failed: {error}");
                                break;
                            }
                            Err(error) => {
                                eprintln!("{TAG} outbound snapshot task failed: {error}");
                                break;
                            }
                        }
                        if let Some(remote_vector) = pending_remote_vector.take() {
                            let policy_bound =
                                sync_policy::is_policy_bound(authenticated_tx).await;
                            let snapshot_already_sent =
                                snapshot_ack::was_sent(authenticated_tx).await;
                            snapshot_drive = Some(snapshot_drive::spawn(
                                Arc::clone(storage),
                                client_session.peer_device_id().to_owned(),
                                remote_vector,
                                snapshot_already_sent,
                                policy_bound,
                                tx.clone(),
                                Arc::clone(authenticated_tx),
                            ));
                        }
                    }
                    _ = feed_recovery.tick() => {
                        if let Err(error) = feed_recovery::pump(
                            storage, authenticated_tx, feed_in_flight,
                        ).await {
                            eprintln!("{TAG} periodic committed feed drain failed: {error}");
                        }
                    }
                    session_input = client_session.receive(&mut session_io) => {
                        use crate::client_session::DirectClientSessionInput as SessionInput;
                        let message = match session_input {
                            SessionInput::Message(message) => *message,
                            SessionInput::Ignore => continue,
                            SessionInput::Malformed => {
                                eprintln!("{TAG} ignoring malformed peer sync message");
                                continue;
                            }
                            SessionInput::Failed(error) => {
                                eprintln!("{TAG} peer session transport failed: {error}");
                                break;
                            }
                            SessionInput::Closed => break,
                            SessionInput::Rejected(reason) => {
                                eprintln!(
                                    "{TAG} closing invalid client session message: {reason:?}"
                                );
                                break;
                            }
                        };
                        use crate::client_session::{
                            DirectClientMessage as ClientMessage,
                        };
                        match message {
                                ClientMessage::Hello(hello) => {
                                    if !server_hello::handle(
                                        hello,
                                        server_hello::Context {
                                            local_device_id: device_id,
                                            local_space_id: space_id,
                                            auth_secret,
                                            local_offer: &client_hello.offer,
                                            local_auth_nonce: client_hello.auth_nonce.as_deref(),
                                            peer,
                                            winning_addr: &winning_addr,
                                            stopped: stopped.as_ref(),
                                            tx: &tx,
                                            client_session: &mut client_session,
                                            pending_registry_admission:
                                                &mut pending_registry_admission,
                                        },
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                }

                                ClientMessage::SyncPolicy(update) => {
                                    let crate::client_session::SyncPolicyUpdate {
                                        revision,
                                        digest,
                                        active,
                                        rules,
                                    } = update;
                                    if let Err(error) = sync_policy::accept_and_start(
                                        storage,
                                        authenticated_tx,
                                        client_session.is_ready(),
                                        revision,
                                        &digest,
                                        active,
                                        rules,
                                    )
                                    .await
                                    {
                                        eprintln!("{TAG} peer sync policy rejected: {error}");
                                        break;
                                    }
                                }

                                ClientMessage::VersionVector(remote_vector) => {
                                    if snapshot_drive.is_some() {
                                        pending_remote_vector = Some(remote_vector);
                                        continue;
                                    }

                                    let policy_bound =
                                        sync_policy::is_policy_bound(authenticated_tx).await;
                                    let snapshot_already_sent =
                                        snapshot_ack::was_sent(authenticated_tx).await;
                                    snapshot_drive = Some(snapshot_drive::spawn(
                                        Arc::clone(storage),
                                        client_session.peer_device_id().to_owned(),
                                        remote_vector,
                                        snapshot_already_sent,
                                        policy_bound,
                                        tx.clone(),
                                        Arc::clone(authenticated_tx),
                                    ));
                                }

                                ClientMessage::SyncChanges {
                                    batch_id,
                                    entities,
                                    is_last,
                                    policy_identity,
                                    usage_complete_through,
                                } => {
                                    let context = sync_changes::Context {
                                        storage,
                                        tx: &tx,
                                        peer_device_id: client_session.peer_device_id(),
                                        local_device_id: device_id,
                                        authenticated_tx,
                                        on_change,
                                    };
                                    if let Err(error) = sync_changes::apply(
                                        context, batch_id, entities, is_last,
                                        usage_complete_through,
                                        policy_identity,
                                    ).await {
                                        eprintln!("{TAG} inbound sync batch failed: {error}");
                                        break;
                                    }
                                }

                                ClientMessage::SyncAck { batch_id, policy_identity } => {
                                    if !snapshot_ack::handle(
                                        storage,
                                        client_session.peer_device_id(),
                                        &batch_id,
                                        policy_identity,
                                        authenticated_tx,
                                        &tx,
                                        feed_in_flight,
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                }

                                ClientMessage::LiveChange {
                                    change_id,
                                    entity,
                                    policy_identity,
                                } => {
                                    if let Err(error) = live_messages::handle_live_change(
                                        storage,
                                        authenticated_tx,
                                        device_id,
                                        client_session.peer_device_id(),
                                        &tx,
                                        on_change,
                                        change_id,
                                        entity,
                                        policy_identity,
                                    )
                                    .await
                                    {
                                        eprintln!("{TAG} live change handling failed: {error}");
                                        break;
                                    }
                                }

                                ClientMessage::LiveAck { change_id, policy_identity } => {
                                    if !live_messages::handle_feed_ack_message(
                                        storage,
                                        client_session.peer_device_id(),
                                        &tx,
                                        feed_in_flight,
                                        authenticated_tx,
                                        &change_id,
                                        policy_identity,
                                        false,
                                    )
                                    .await {
                                        break;
                                    }
                                }

                                ClientMessage::LiveReceiptAck {
                                    change_id,
                                    policy_identity,
                                } => {
                                    if !live_messages::handle_feed_ack_message(
                                        storage,
                                        client_session.peer_device_id(),
                                        &tx,
                                        feed_in_flight,
                                        authenticated_tx,
                                        &change_id,
                                        policy_identity,
                                        true,
                                    )
                                    .await {
                                        break;
                                    }
                                }

                                ClientMessage::TaskMergeBatch(batch) => {
                                    if let Err(error) = live_messages::handle_task_merge_batch(
                                        storage,
                                        client_session.peer_device_id(),
                                        device_id,
                                        &tx,
                                        authenticated_tx,
                                        batch,
                                    )
                                    .await
                                    {
                                        eprintln!("{TAG} task merge batch rejected: {error}");
                                    }
                                }

                                ClientMessage::TaskMergeBatchAck => {}

                                ClientMessage::Ping { ts } => {
                                    let _ = send_msg(&tx, LanSyncMessage::Pong { ts }).await;
                                }
                                ClientMessage::Pong
                                | ClientMessage::PairingRejected
                                | ClientMessage::PairingAccepted => {}
                                ClientMessage::PeerList(peers) => {
                                    if client_session.is_ready() {
                                        if let Some(handler) =
                                            on_peer_list.lock().await.as_ref()
                                        {
                                            handler(peers);
                                        }
                                    }
                                }
                                ClientMessage::ProtocolRejected {
                                    origin_device_id,
                                    target_device_id,
                                    reason,
                                } => {
                                    let authenticated_peer_id = client_session.peer_device_id();
                                    if target_device_id == device_id.as_str()
                                        && (authenticated_peer_id.is_empty()
                                            || origin_device_id == authenticated_peer_id)
                                    {
                                        eprintln!("{TAG} Peer rejected protocol: {reason}");
                                        break;
                                    }
                                }
                                ClientMessage::RegistryControl(sync_msg) => {
                                    if !registry_control::handle(
                                        sync_msg,
                                        registry_control::Context {
                                            pending_registry_admission:
                                                &mut pending_registry_admission,
                                            device_id,
                                            storage,
                                            tx: &tx,
                                            authenticated_tx,
                                            peer,
                                            on_registry_events,
                                            on_connected,
                                            client_session: &mut client_session,
                                            stopped: stopped.as_ref(),
                                        },
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                    continue;
                                }
                        }
                    }
                }
            }

            if let Some(task) = snapshot_drive.take() {
                task.abort();
                let _ = task.await;
            }
            if let Err(error) = session_io.finish().await {
                eprintln!("{TAG} peer session transport shutdown failed: {error:?}");
            }
            *authenticated_tx.lock().await = None;

            if client_session.is_ready() {
                eprintln!("{TAG} Disconnected from peer");
                if let Some(handler) = on_disconnected.lock().await.as_ref() {
                    handler(client_session.peer_device_id().to_owned());
                }
            }
        }
        Ok(None) => {
            eprintln!("{TAG} All addresses failed for {}", peer_record.device_name);
        }
        Err(error) => {
            eprintln!("{TAG} could not start peer session transport: {error:?}");
        }
    }

    if stopped.load(std::sync::atomic::Ordering::Relaxed) {
        return Disposition::Stop;
    }
    Disposition::Backoff
}
