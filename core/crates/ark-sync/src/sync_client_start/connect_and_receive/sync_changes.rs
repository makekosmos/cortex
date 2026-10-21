use super::*;

pub(super) struct Context<'a> {
    pub storage: &'a Arc<dyn SyncRuntimeStorage>,
    pub tx: &'a ClientMessageSender,
    pub peer_device_id: &'a str,
    pub local_device_id: &'a str,
    pub authenticated_tx: &'a AuthenticatedTx,
    pub on_change: &'a Arc<Mutex<Option<ClientOnChangeCallback>>>,
}

pub(super) async fn apply(
    context: Context<'_>,
    batch_id: String,
    entities: Vec<SyncEntity>,
    is_last: bool,
    usage_complete_through: Option<std::collections::HashMap<String, u64>>,
    policy_identity: Option<ark_application::sync_storage::PolicyIdentity>,
) -> Result<(), String> {
    let Context {
        storage,
        tx,
        peer_device_id,
        local_device_id,
        authenticated_tx,
        on_change,
    } = context;
    let policy_bound = sync_policy::is_policy_bound(authenticated_tx).await;
    sync_policy::validate_snapshot_identity(
        storage,
        local_device_id,
        policy_bound,
        &policy_identity,
    )
    .await?;
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
            .apply_authenticated_entity_with_policy(
                entity,
                peer_device_id,
                &ark_application::sync_storage::entity_inbox_message_id(
                    peer_device_id,
                    &entity.entity_type,
                    &entity.id,
                    &entity.hlc,
                ),
                local_device_id,
                policy_identity.clone(),
            )
            .await
        {
            Ok(ark_application::sync_storage::InboundApplyOutcome::Applied)
            | Ok(ark_application::sync_storage::InboundApplyOutcome::Stale) => {
                vector_updated |= observe_non_usage_entity(&mut local_vector, entity);
                accepted += 1;
                if let Some(handler) = on_change.lock().await.as_ref() {
                    handler(entity.clone());
                }
            }
            Ok(ark_application::sync_storage::InboundApplyOutcome::Pending) => {}
            Err(error) => eprintln!(
                "{TAG} Failed to apply sync entity {}:{}: {error}",
                entity.entity_type, entity.id
            ),
        }
    }

    match policy_identity.as_ref() {
        Some(identity) => {
            crate::inbound_snapshot::persist_pull_vector_with_policy(
                storage.as_ref(),
                local_device_id,
                identity,
                &mut local_vector,
                vector_updated,
                is_last,
                usage_complete_through.as_ref(),
            )
            .await?;
        }
        None => {
            crate::inbound_snapshot::persist_pull_vector(
                storage.as_ref(),
                &mut local_vector,
                vector_updated,
                is_last,
                usage_complete_through.as_ref(),
            )
            .await?;
        }
    }
    send_msg(
        tx,
        LanSyncMessage::SyncAck {
            batch_id,
            accepted,
            policy_identity,
        },
    )
    .await?;
    if is_last {
        eprintln!("{TAG} Received all sync batches from server");
    }
    Ok(())
}
