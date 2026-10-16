use super::*;

pub(super) async fn send(
    storage: &Arc<dyn SyncRuntimeStorage>,
    tx: &mpsc::UnboundedSender<Message>,
    peer_device_id: &str,
    remote_vector: &VersionVector,
    sync_complete: bool,
) -> Result<String, String> {
    let mut local_vector = load_version_vector(storage).await?;
    // KOS-302: snapshot completeness claims before the first page. A claim
    // computed later could include a sequence allocated mid-pull that the peer
    // never received.
    let read_class = crate::application::sync_storage::SyncReadClass::for_sync_state(sync_complete);
    let complete_through = storage
        .usage_complete_through_for_peer(peer_device_id, read_class)
        .await?;
    let mut offset = 0;
    loop {
        let mut load_vector = local_vector.clone();
        merge_usage_cursors(&mut load_vector, remote_vector);
        let entities = storage
            .load_entities_page_for_peer(
                peer_device_id,
                read_class,
                &load_vector,
                offset,
                SYNC_LOAD_PAGE_SIZE,
            )
            .await?;
        if entities.is_empty() {
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
        for batch in split_into_batches(&to_send) {
            send_msg(
                tx,
                &LanSyncMessage::SyncChanges {
                    batch_id: generate_id(),
                    entities: batch,
                    is_last: false,
                    origin_device_id: None,
                    usage_complete_through: None,
                },
            );
        }
    }
    save_version_vector(storage, &local_vector).await?;
    let terminal_batch_id = generate_id();
    send_msg(
        tx,
        &LanSyncMessage::SyncChanges {
            batch_id: terminal_batch_id.clone(),
            entities: vec![],
            is_last: true,
            origin_device_id: None,
            usage_complete_through: (!complete_through.is_empty()).then_some(complete_through),
        },
    );
    Ok(terminal_batch_id)
}
