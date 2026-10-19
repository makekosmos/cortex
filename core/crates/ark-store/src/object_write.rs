use ark_application::object_write::{NewObjectReceipt, ObjectWriteSession, StoredObjectReceipt};
use ark_domain::canonical_type::{
    prepare_resolved_object, CanonicalIngressError, LEGACY_TASK_TYPE,
};
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ArkObjectWrite, ObjectLink};
use rusqlite::Connection;

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
        let identity = crate::objects::object_type_identity(self.connection, id)?;
        let object_revision = crate::sync_versions::object_revision(self.connection, id)?;
        let task_revision = crate::task_merge::task_snapshot_token(self.connection, id)
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
