use super::*;

pub(super) async fn send(
    storage: &Arc<dyn SyncRuntimeStorage>,
    tx: &mpsc::UnboundedSender<Message>,
    peer_device_id: &str,
    remote_vector: &VersionVector,
    sync_complete: bool,
    mut policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<
    (
        String,
        Option<ark_application::sync_storage::PolicyIdentity>,
    ),
    String,
> {
    let mut local_vector = load_version_vector(storage).await?;
    let initial_local_vector = local_vector.clone();
    // KOS-302: snapshot completeness claims before the first page. A claim
    // computed later could include a sequence allocated mid-pull that the peer
    // never received.
    let read_class = ark_application::sync_storage::SyncReadClass::for_sync_state(sync_complete);
    let complete_through = storage
        .usage_complete_through_for_peer(peer_device_id, read_class)
        .await?;
    let mut offset = 0;
    'snapshot: loop {
        if policy_identity.is_some() {
            let current = storage
                .accepted_policy_identity_for_peer(peer_device_id)
                .await?
                .ok_or_else(|| {
                    "accepted_policy_identity_missing_for_negotiated_peer".to_string()
                })?;
            if Some(current.clone()) != policy_identity {
                policy_identity = Some(current);
                local_vector.clone_from(&initial_local_vector);
                offset = 0;
            }
        }
        let mut load_vector = local_vector.clone();
        merge_usage_cursors(&mut load_vector, remote_vector);
        let entities = match policy_identity.as_ref() {
            Some(identity) => match storage
                .load_policy_bound_entities_page_for_peer(
                    peer_device_id,
                    read_class,
                    &load_vector,
                    offset,
                    SYNC_LOAD_PAGE_SIZE,
                    identity,
                )
                .await
            {
                Ok(page) => page.entities,
                Err(error) if error.contains("stale_or_unnegotiated_sync_policy") => continue,
                Err(error) => return Err(error),
            },
            None => {
                storage
                    .load_entities_page_for_peer(
                        peer_device_id,
                        read_class,
                        &load_vector,
                        offset,
                        SYNC_LOAD_PAGE_SIZE,
                    )
                    .await?
            }
        };
        if entities.is_empty() {
            if policy_identity.is_some() {
                let current = storage
                    .accepted_policy_identity_for_peer(peer_device_id)
                    .await?
                    .ok_or_else(|| {
                        "accepted_policy_identity_missing_for_negotiated_peer".to_string()
                    })?;
                if Some(current.clone()) != policy_identity {
                    policy_identity = Some(current);
                    local_vector.clone_from(&initial_local_vector);
                    offset = 0;
                    continue 'snapshot;
                }
            }
            break;
        }
        offset += entities.len();
        let mut to_send = Vec::new();
        for entity in entities {
            if !is_usage_entity(&entity) && !local_vector.contains_key(&entity.id) {
                local_vector.insert(entity.id.clone(), entity.hlc.clone());
            }
            if should_send_entity(remote_vector, &entity) {
                to_send.push(entity);
            }
        }
        if policy_identity.is_some()
            && storage
                .accepted_policy_identity_for_peer(peer_device_id)
                .await?
                != policy_identity
        {
            policy_identity = storage
                .accepted_policy_identity_for_peer(peer_device_id)
                .await?;
            if policy_identity.is_none() {
                return Err("accepted_policy_identity_missing_for_negotiated_peer".into());
            }
            local_vector.clone_from(&initial_local_vector);
            offset = 0;
            continue 'snapshot;
        }
        for batch in split_into_batches(&to_send) {
            send_msg(
                tx,
                &LanSyncMessage::SyncChanges {
                    policy_identity: policy_identity.clone(),
                    batch_id: generate_id(),
                    entities: batch,
                    is_last: false,
                    origin_device_id: None,
                    usage_complete_through: None,
                },
            );
        }
    }
    if policy_identity.is_some()
        && storage
            .accepted_policy_identity_for_peer(peer_device_id)
            .await?
            != policy_identity
    {
        return Err("outbound_snapshot_policy_changed_before_terminal_page".into());
    }
    save_version_vector(storage, &local_vector).await?;
    let terminal_batch_id = generate_id();
    send_msg(
        tx,
        &LanSyncMessage::SyncChanges {
            policy_identity: policy_identity.clone(),
            batch_id: terminal_batch_id.clone(),
            entities: vec![],
            is_last: true,
            origin_device_id: None,
            usage_complete_through: (!complete_through.is_empty()).then_some(complete_through),
        },
    );
    Ok((terminal_batch_id, policy_identity))
}
