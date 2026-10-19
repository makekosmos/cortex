use super::TaskMergeError;
use ark_domain::task_merge::TaskMergeBootstrap;
use rusqlite::Connection;

pub(super) fn append_bootstrap(
    transaction: &Connection,
    bootstrap: TaskMergeBootstrap,
) -> Result<bool, TaskMergeError> {
    let object_id = bootstrap.object.id.clone();
    let prepared = match ark_store::task_merge::check_bootstrap(transaction, &bootstrap)? {
        ark_store::task_merge::TaskMergeBootstrapCheck::Replay => {
            ark_store::task_merge::materialize_task_if_ready(transaction, &object_id)?;
            return Ok(true);
        }
        ark_store::task_merge::TaskMergeBootstrapCheck::New(prepared) => prepared,
    };
    let bootstrap = prepared.bootstrap().clone();

    let existing_object =
        crate::db::get_object(transaction, &bootstrap.object.id).map_err(storage_error)?;
    if let Some(existing_object) = existing_object {
        let mut existing_links = crate::db::list_object_links(transaction)
            .map_err(storage_error)?
            .into_iter()
            .filter(|link| link.source_object_id == bootstrap.object.id)
            .collect::<Vec<_>>();
        existing_links.sort_by(|left, right| left.id.cmp(&right.id));
        let mut incoming_links = bootstrap.links.clone();
        incoming_links.sort_by(|left, right| left.id.cmp(&right.id));
        if existing_object != bootstrap.object || existing_links != incoming_links {
            return Err(TaskMergeError::Invalid(
                "task_merge_bootstrap_existing_object_mismatch".into(),
            ));
        }
    }

    ark_store::task_merge::persist_bootstrap(transaction, prepared)?;
    crate::db::upsert_object(transaction, &bootstrap.object).map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM object_links WHERE source_object_id = ?1",
            [&bootstrap.object.id],
        )
        .map_err(storage_error)?;
    for link in &bootstrap.links {
        crate::db::upsert_object_link(transaction, link).map_err(storage_error)?;
    }

    crate::db::record_remote_sync_revision(
        transaction,
        &bootstrap.object.id,
        &bootstrap.source_hlc,
    )
    .map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc,
                 deleted = excluded.deleted
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![
                bootstrap.object.id,
                bootstrap.source_hlc,
                bootstrap.object.deleted_at.is_some(),
            ],
        )
        .map_err(storage_error)?;
    if bootstrap.object.deleted_at.is_none() {
        crate::db::delete_sync_tombstone(transaction, &bootstrap.object.id)
            .map_err(storage_error)?;
    }

    ark_store::task_merge::materialize_task_if_ready(transaction, &bootstrap.object.id)?;
    Ok(false)
}

fn storage_error(error: impl std::fmt::Display) -> TaskMergeError {
    TaskMergeError::Storage(error.to_string())
}
