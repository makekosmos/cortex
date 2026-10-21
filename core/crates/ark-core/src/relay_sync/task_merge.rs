use super::*;

pub(super) async fn handle_message(
    relay: &RelaySync,
    from_device_id: &str,
    transport_public_key: Option<&str>,
    message: LanSyncMessage,
) {
    match message {
        LanSyncMessage::TaskMergeBatch { batch } => {
            if let Err(error) = relay
                .apply_task_merge_batch(from_device_id, transport_public_key, batch)
                .await
            {
                eprintln!("{TAG} task merge batch rejected from {from_device_id}: {error}");
            }
        }
        LanSyncMessage::TaskMergeBatchAck {
            batch_id,
            through_position,
            policy_identity,
        } => {
            if let Err(error) = relay
                .acknowledge_task_merge_batch(
                    from_device_id,
                    &batch_id,
                    through_position,
                    &policy_identity,
                )
                .await
            {
                eprintln!("{TAG} task merge ACK rejected from {from_device_id}: {error}");
            }
        }
        _ => (),
    }
}
