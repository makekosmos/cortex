use crate::types::SyncEntity;
use rusqlite::Connection;

pub fn apply_remote_task_delete(
    conn: &Connection,
    entity: &SyncEntity,
) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
    conn.execute_batch("SAVEPOINT canonical_sync_task_delete")
        .map_err(|error| error.to_string())?;
    let session = crate::db::SqliteObjectWriteSession::new(conn);
    let result = crate::application::task_mutation::apply_inbound_task_mutation(
        &session,
        crate::application::task_mutation::InboundTaskMutation::RemoteHardDelete { entity },
    )
    .map_err(|error| error.to_string());
    match result {
        Ok(outcome) => {
            conn.execute_batch("RELEASE canonical_sync_task_delete")
                .map_err(|error| error.to_string())?;
            Ok(match outcome {
                crate::application::task_mutation::InboundTaskMutationOutcome::Applied => {
                    ark_application::sync_storage::InboundApplyOutcome::Applied
                }
                crate::application::task_mutation::InboundTaskMutationOutcome::Stale => {
                    ark_application::sync_storage::InboundApplyOutcome::Stale
                }
            })
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO canonical_sync_task_delete; RELEASE canonical_sync_task_delete",
            );
            Err(error)
        }
    }
}
