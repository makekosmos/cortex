use crate::canonical_types::game::{GameMutationResult, GameUpsertCommand};
use crate::types::ArkObject;

pub use ark_application::object_mutation::{
    LocalObjectDelete, LocalObjectUpsert, ObjectDeleteCommit, ObjectRevisionKind,
    ObjectUpsertCommit,
};

pub trait ObjectMutationSession {
    fn persist_object(&self, object: &ArkObject) -> Result<(), String>;
    fn record_upsert(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
    ) -> Result<String, String>;
    fn record_delete(
        &self,
        kind: ObjectRevisionKind,
        id: &str,
        device_id: &str,
        type_id: Option<&str>,
    ) -> Result<String, String>;
    fn write_object_sync_version(
        &self,
        id: &str,
        hlc: &str,
        deleted: bool,
    ) -> Result<usize, String>;
    fn upsert_game(
        &self,
        command: GameUpsertCommand,
        fallback_device_id: &str,
    ) -> Result<GameMutationResult, String>;
}

pub fn apply_game_upsert(
    session: &dyn ObjectMutationSession,
    command: GameUpsertCommand,
    fallback_device_id: &str,
) -> Result<GameMutationResult, String> {
    session.upsert_game(command, fallback_device_id)
}
