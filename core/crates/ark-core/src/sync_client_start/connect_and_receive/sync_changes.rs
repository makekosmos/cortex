use super::*;

pub(super) struct Context<'a> {
    pub storage: &'a Arc<dyn SyncRuntimeStorage>,
    pub tx: &'a mpsc::UnboundedSender<Message>,
    pub peer_device_id: &'a str,
    pub on_change: &'a Arc<Mutex<Option<ClientOnChangeCallback>>>,
}

pub(super) async fn apply(
    context: Context<'_>,
    batch_id: String,
    entities: Vec<SyncEntity>,
    is_last: bool,
    usage_complete_through: Option<std::collections::HashMap<String, u64>>,
) -> Result<(), String> {
    let Context {
        storage,
        tx,
        peer_device_id,
        on_change,
    } = context;
    let mut local_vector = load_version_vector(storage).await?;
    let mut accepted = 0;
    let mut vector_updated = false;

    for entity in &entities {
        let should_apply = is_usage_entity(entity)
            || local_vector
                .get(&entity.id)
                .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc));
        if !should_apply {
            continue;
        }
        match storage
            .apply_authenticated_entity(entity, peer_device_id)
            .await
        {
            Ok(()) => {
                vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                accepted += 1;
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity.clone());
                }
            }
            Err(error) => eprintln!(
                "{TAG} Failed to apply sync entity {}:{}: {error}",
                entity.entity_type, entity.id
            ),
        }
    }

    crate::sync_server::persist_pull_vector(
        storage,
        &mut local_vector,
        vector_updated,
        is_last,
        usage_complete_through.as_ref(),
    )
    .await?;
    send_msg(tx, &LanSyncMessage::SyncAck { batch_id, accepted });
    if is_last {
        eprintln!("{TAG} Received all sync batches from server");
    }
    Ok(())
}
