use super::TaskMergeError;
use ark_domain::task_merge::TaskMergeBootstrap;
use rusqlite::Connection;

pub(super) fn append_bootstrap(
    transaction: &Connection,
    bootstrap: TaskMergeBootstrap,
) -> Result<bool, TaskMergeError> {
    ark_store::task_merge::apply_inbound_bootstrap(transaction, &bootstrap).map_err(Into::into)
}
