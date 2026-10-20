use super::super::DbError;
use ark_application::sync_storage::PolicyIdentity;
use rusqlite::Transaction;

pub(crate) fn require_sync_policy_identity(
    transaction: &Transaction<'_>,
    local_device_id: Option<&str>,
    expected: Option<&PolicyIdentity>,
) -> Result<(), DbError> {
    if let Some(expected) = expected {
        let local_device_id = local_device_id.ok_or_else(|| {
            DbError::Permission("sync_policy_identity_missing_local_device".into())
        })?;
        let profile =
            ark_store::data_platform::load_sync_profile_for_sync(transaction, local_device_id)
                .map_err(DbError::from_application_error)?;
        let actual = ark_application::sync_policy::sync_profile_identity(profile.as_ref());
        if &actual != expected {
            return Err(DbError::Permission(
                "stale_or_unnegotiated_sync_policy".into(),
            ));
        }
    }
    Ok(())
}
