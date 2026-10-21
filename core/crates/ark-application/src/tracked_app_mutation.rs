use ark_domain::types::{SyncEntity, TrackedApp};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TrackedAppUpsert {
    pub tracked_app: TrackedApp,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackedAppDelete {
    pub id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum TrackedAppMutation {
    Upsert(Box<TrackedAppUpsert>),
    Delete(TrackedAppDelete),
}

pub trait TrackedAppMutationSession {
    fn apply(&self, mutation: TrackedAppMutation) -> Result<SyncEntity, String>;
}

pub fn apply_tracked_app_mutation(
    session: &dyn TrackedAppMutationSession,
    mutation: TrackedAppMutation,
) -> Result<SyncEntity, String> {
    session.apply(mutation)
}
