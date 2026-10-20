use ark_application::object_write::{NewObjectReceipt, ObjectWriteSession, StoredObjectReceipt};
use ark_domain::canonical_type::{
    prepare_resolved_object, CanonicalIngressError, LEGACY_TASK_TYPE,
};
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ArkObjectWrite, ObjectLink};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::Serialize;

/// Resolve the persisted type identity and validate an object write before
/// callers perform any persistence or synchronization mutation.
pub fn prepare_object(
    connection: &Connection,
    input: ArkObjectWrite,
) -> Result<ArkObject, CanonicalIngressError> {
    let input_type_id = input.type_id.clone();
    if input_type_id == LEGACY_TASK_TYPE {
        return Err(CanonicalIngressError::invalid("legacy_task_type_retired"));
    }
    let requested_version = input.type_version.clone();
    let (canonical_type_id, resolved_version) = crate::type_registry::resolve_object_type_identity(
        connection,
        &input_type_id,
        requested_version.as_deref(),
    )
    .map_err(|_| CanonicalIngressError::invalid("unknown_type_or_version"))?;
    let registrations = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .map_err(|_| CanonicalIngressError::invariant("canonical_registry"))?;
    let registration = registrations.iter().find(|registration| {
        registration.type_id == canonical_type_id && registration.version == resolved_version
    });

    prepare_resolved_object(input, &canonical_type_id, &resolved_version, registration)
}

/// Execute one local object upsert and its receipt in one Store-owned transaction.
pub fn apply_local_object_upsert(
    connection: &mut Connection,
    write: ark_application::object_mutation::LocalObjectUpsert,
    principal: &str,
    operation_id: Option<&str>,
) -> Result<
    ark_application::object_mutation::ObjectUpsertOutcome,
    crate::writer::TransactionFailure<ark_application::object_mutation::ObjectUpsertError>,
> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| {
            crate::writer::TransactionFailure::Begin(crate::writer::store_error_from_sqlite(error))
        })?;
    let outcome = apply_local_object_upsert_in(&transaction, write, principal, operation_id)
        .map_err(crate::writer::TransactionFailure::Operation)?;
    transaction.commit().map_err(|error| {
        crate::writer::TransactionFailure::Commit(crate::writer::store_error_from_sqlite(error))
    })?;
    Ok(outcome)
}

pub(crate) fn apply_local_object_upsert_in(
    transaction: &Transaction<'_>,
    write: ark_application::object_mutation::LocalObjectUpsert,
    principal: &str,
    operation_id: Option<&str>,
) -> Result<
    ark_application::object_mutation::ObjectUpsertOutcome,
    ark_application::object_mutation::ObjectUpsertError,
> {
    use ark_application::object_mutation::{
        build_local_object_upsert_commit, validate_local_object_upsert,
        validate_local_object_upsert_type, ObjectUpsertError, ObjectUpsertOutcome,
    };

    const COMMAND_TYPE: &str = "object.upsert.v1";
    let payload_sha256 = operation_id
        .map(|_| operation_payload_hash(COMMAND_TYPE, &write))
        .transpose()
        .map_err(ObjectUpsertError::Storage)?;

    if let (Some(operation_id), Some(payload_sha256)) = (operation_id, payload_sha256.as_deref()) {
        if let Some(receipt) = crate::operation_receipt::load(transaction, principal, operation_id)
            .map_err(|error| ObjectUpsertError::Storage(error.to_string()))?
        {
            if receipt.command_type != COMMAND_TYPE || receipt.payload_sha256 != payload_sha256 {
                return Err(ObjectUpsertError::OperationIdConflict);
            }
            let value = serde_json::from_str(&receipt.result_json)
                .map_err(|error| ObjectUpsertError::Storage(error.to_string()))?;
            return Ok(ObjectUpsertOutcome {
                value,
                replayed: true,
            });
        }
    }

    validate_local_object_upsert_type(&write).map_err(ObjectUpsertError::Conflict)?;
    let current_snapshot = write
        .expected_snapshot
        .as_ref()
        .map(|_| read_object_write_snapshot(transaction, &write.object.id))
        .transpose()
        .map_err(ObjectUpsertError::Storage)?;
    validate_local_object_upsert(&write, current_snapshot.as_ref())
        .map_err(ObjectUpsertError::Conflict)?;

    let object = prepare_object(transaction, write.object)
        .map_err(|error| ObjectUpsertError::Storage(error.to_string()))?;
    crate::objects::upsert_object(transaction, &object).map_err(ObjectUpsertError::Storage)?;
    let hlc = crate::sync_versions::bump_sync_version_vector(
        transaction,
        "object",
        &object.id,
        &write.device_id,
        false,
    )
    .map_err(ObjectUpsertError::Storage)?;
    crate::sync_tombstones::delete(transaction, &object.id).map_err(ObjectUpsertError::Storage)?;
    let outcome = ObjectUpsertOutcome {
        value: build_local_object_upsert_commit(object, hlc),
        replayed: false,
    };

    if let (Some(operation_id), Some(payload_sha256)) = (operation_id, payload_sha256) {
        let result_json = serde_json::to_string(&outcome.value)
            .map_err(|error| ObjectUpsertError::Storage(error.to_string()))?;
        crate::operation_receipt::insert(
            transaction,
            principal,
            operation_id,
            &crate::operation_receipt::OperationReceipt {
                command_type: COMMAND_TYPE.to_owned(),
                payload_sha256,
                result_json,
            },
        )
        .map_err(|error| ObjectUpsertError::Storage(error.to_string()))?;
    }

    Ok(outcome)
}

pub fn read_object_write_snapshot(
    connection: &Connection,
    object_id: &str,
) -> Result<ObjectWriteSnapshot, String> {
    let identity = crate::objects::object_type_identity(connection, object_id)?;
    let object_revision = crate::sync_versions::object_revision(connection, object_id)?;
    let task_revision = crate::task_merge::task_snapshot_token(connection, object_id)
        .map_err(|error| error.to_string())?;
    Ok(match identity {
        Some(identity) => {
            let is_task = identity.type_id == "core.task";
            ObjectWriteSnapshot {
                exists: true,
                type_id: Some(identity.type_id),
                type_version: Some(identity.type_version),
                revision: if is_task {
                    task_revision.or(object_revision)
                } else {
                    object_revision
                },
            }
        }
        None => ObjectWriteSnapshot {
            exists: false,
            type_id: None,
            type_version: None,
            revision: task_revision.or(object_revision),
        },
    })
}

fn operation_payload_hash(command_type: &str, command: &impl Serialize) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    #[derive(Serialize)]
    struct Command<'a, T> {
        write: &'a T,
    }

    let command = Command { write: command };
    let payload = serde_json::to_value(("ark-operation-v1", command_type, command))
        .map_err(|error| error.to_string())?;
    let canonical_payload = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical_payload)))
}

pub struct SqliteObjectWriteSession<'connection> {
    connection: &'connection Connection,
}

impl<'connection> SqliteObjectWriteSession<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }
}

impl ObjectWriteSession for SqliteObjectWriteSession<'_> {
    fn resolve_type_identity(
        &self,
        type_id: &str,
        requested_version: Option<&str>,
    ) -> Result<(String, String), String> {
        crate::type_registry::resolve_object_type_identity(
            self.connection,
            type_id,
            requested_version,
        )
    }

    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        read_object_write_snapshot(self.connection, id)
    }

    fn current_revision(&self, id: &str) -> Result<Option<String>, String> {
        crate::sync_versions::object_revision(self.connection, id)
    }

    fn is_peer_removed(&self, peer_id: &str) -> Result<bool, String> {
        let removed: Vec<String> =
            crate::sync_kv::get_sync_kv(self.connection, "sync.removed_peers")?
                .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
                .transpose()?
                .unwrap_or_default();
        Ok(removed.iter().any(|removed_id| removed_id == peer_id))
    }

    fn persist_object(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String> {
        crate::objects::upsert_object(self.connection, object)?;
        for link in links {
            crate::objects::upsert_object_link(self.connection, link)?;
        }
        Ok(())
    }

    fn record_local_revision(&self, id: &str, device_id: Option<&str>) -> Result<String, String> {
        let device_id = device_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or("ark-core-rpc-local");
        crate::sync_versions::bump_local_revision(
            self.connection,
            crate::sync_versions::LocalRevisionTarget::Object,
            id,
            device_id,
        )
    }

    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        crate::sync_versions::record_remote_object_revision(self.connection, id, source_hlc, false)
    }

    fn load_receipt(
        &self,
        entity_id: &str,
        representation_kind: &str,
        source_hlc: &str,
    ) -> Result<Option<StoredObjectReceipt>, String> {
        crate::object_write_receipts::load_receipt(
            self.connection,
            entity_id,
            representation_kind,
            source_hlc,
        )
    }

    fn insert_receipt(&self, receipt: NewObjectReceipt) -> Result<StoredObjectReceipt, String> {
        crate::object_write_receipts::insert_receipt(self.connection, receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_object;
    use ark_domain::types::ArkObjectWrite;
    use rusqlite::Connection;
    use serde_json::json;

    #[test]
    fn prepare_object_resolves_legacy_alias_without_core() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(concat!(
                "CREATE TABLE object_types(id TEXT PRIMARY KEY, ",
                "current_version TEXT NOT NULL); ",
                "CREATE TABLE object_type_aliases(alias TEXT PRIMARY KEY, ",
                "canonical_type_id TEXT NOT NULL); ",
                "CREATE TABLE object_type_versions(type_id TEXT NOT NULL, ",
                "version TEXT NOT NULL); ",
                "INSERT INTO object_types(id, current_version) VALUES ",
                "('note.canonical', '1.2.0'); ",
                "INSERT INTO object_type_aliases(alias, canonical_type_id) ",
                "VALUES ('note.legacy', 'note.canonical'); ",
                "INSERT INTO object_type_versions(type_id, version) VALUES ",
                "('note.canonical', '1.2.0');"
            ))
            .expect("type registry fixture is created");

        let object = prepare_object(
            &connection,
            ArkObjectWrite {
                id: "note-1".into(),
                type_id: "note.legacy".into(),
                type_version: Some("0.0.0-legacy".into()),
                title: "A note".into(),
                content_json: json!({}),
                props_json: json!({}),
                created_at: "2026-10-19T00:00:00Z".into(),
                updated_at: "2026-10-19T00:00:00Z".into(),
                deleted_at: None,
            },
        )
        .expect("the Store resolves and prepares the legacy write");

        assert_eq!(object.type_id, "note.canonical");
        assert_eq!(object.type_version, "1.2.0");
    }

    #[test]
    fn prepare_object_rejects_a_new_write_of_the_legacy_task_type() {
        let connection = Connection::open_in_memory().expect("database opens");
        crate::open::init_schema(&connection).expect("database schema initializes");
        let write = ArkObjectWrite {
            id: "new-legacy-task".into(),
            type_id: "com.kosmos.task".into(),
            type_version: Some("1.1.0".into()),
            title: "Old writer".into(),
            content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
            props_json: json!({"status":"todo"}),
            created_at: "2026-10-13T00:00:00.000Z".into(),
            updated_at: "2026-10-13T00:00:00.000Z".into(),
            deleted_at: None,
        };

        let error = prepare_object(&connection, write).expect_err("legacy writes are retired");
        assert!(error.to_string().contains("legacy_task_type_retired"));
    }
}
