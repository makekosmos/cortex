#[cfg(debug_assertions)]
use super::Connection;
use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};
use ark_domain::device_registry::{RegistryApplyOutcome, RegistryEvent};
use rusqlite::Transaction;

#[derive(Debug)]
pub enum RegistryEventApplyError {
    Permission(String),
    Conflict(String),
    Storage(String),
}

pub struct DeviceRegistryWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn device_registry(&mut self) -> DeviceRegistryWrites<'_, '_> {
        DeviceRegistryWrites {
            transaction: self.transaction,
        }
    }
}

impl DeviceRegistryWrites<'_, '_> {
    pub fn apply_events(
        &mut self,
        sender_device_id: &str,
        events: &[RegistryEvent],
    ) -> Result<ark_application::replication_storage::RegistryPageApply, RegistryEventApplyError>
    {
        let transaction = self.transaction;
        // Known revoked peers may still deliver registry control events; ordinary sync data has
        // a separate admission check in the same Store transaction path.
        if !crate::device_registry::is_device_known(transaction, sender_device_id)
            .map_err(RegistryEventApplyError::Storage)?
        {
            return Err(RegistryEventApplyError::Permission(
                "sync_peer_not_registered".into(),
            ));
        }

        let mut registry = crate::device_registry::load_registry(transaction)
            .map_err(RegistryEventApplyError::Storage)?;
        let mut applied_events = Vec::new();
        for event in events {
            let event_json = serde_json::to_string(event)
                .map_err(|error| RegistryEventApplyError::Storage(error.to_string()))?;
            let existing = crate::device_registry::event_json_for_actor_sequence(
                transaction,
                event.actor_device_id().as_str(),
                event.actor_sequence(),
            )
            .map_err(RegistryEventApplyError::Storage)?;
            if let Some(existing) = existing {
                if existing == event_json {
                    continue;
                }
                return Err(RegistryEventApplyError::Conflict(
                    "device_registry_sequence_conflict".into(),
                ));
            }

            let outcome = registry
                .apply(event.clone())
                .map_err(|error| RegistryEventApplyError::Permission(error.to_string()))?;
            let RegistryApplyOutcome::Applied { event_hash } = outcome;
            let target = registry.device(event.target_device_id()).ok_or_else(|| {
                RegistryEventApplyError::Storage(
                    "device_registry_target_missing_after_apply".into(),
                )
            })?;
            i64::try_from(event.actor_sequence()).map_err(|_| {
                RegistryEventApplyError::Conflict("device_registry_sequence_exhausted".into())
            })?;
            crate::device_registry::persist_event_and_projection(
                transaction,
                event,
                &event_hash,
                &event_json,
                target,
            )
            .map_err(RegistryEventApplyError::Storage)?;
            applied_events.push(event.clone());
        }

        let disabled_device_ids = applied_events
            .iter()
            .filter(|event| {
                matches!(
                    event.operation(),
                    ark_domain::device_registry::RegistryOperation::Disable
                )
            })
            .map(|event| event.target_device_id().as_str().to_owned())
            .collect();
        let sender_is_admitted =
            crate::device_registry::is_device_admitted(transaction, sender_device_id)
                .map_err(RegistryEventApplyError::Storage)?
                .unwrap_or(false);
        let local_device_id = crate::sync_kv::get_sync_kv(
            transaction,
            crate::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY,
        )
        .map_err(RegistryEventApplyError::Storage)?;
        let local_device_is_admitted = local_device_id
            .as_deref()
            .map(|device_id| {
                crate::device_registry::is_device_admitted(transaction, device_id)
                    .map(|admitted| admitted.unwrap_or(true))
                    .map_err(RegistryEventApplyError::Storage)
            })
            .transpose()?
            .unwrap_or(true);
        let digest = crate::device_registry::registry_digest(transaction)
            .map_err(RegistryEventApplyError::Storage)?;

        Ok(ark_application::replication_storage::RegistryPageApply {
            digest,
            sender_is_admitted,
            local_device_is_admitted,
            disabled_device_ids,
            applied_events,
        })
    }
}

impl SqliteWriter {
    pub fn apply_device_registry_events(
        &mut self,
        sender_device_id: &str,
        events: &[RegistryEvent],
    ) -> Result<
        ark_application::replication_storage::RegistryPageApply,
        StoreTransactionFailure<RegistryEventApplyError>,
    > {
        self.write_with_capabilities(|capabilities| {
            capabilities
                .device_registry()
                .apply_events(sender_device_id, events)
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_device_registry_events_on_connection(
        connection: &mut Connection,
        sender_device_id: &str,
        events: &[RegistryEvent],
    ) -> Result<
        ark_application::replication_storage::RegistryPageApply,
        StoreTransactionFailure<RegistryEventApplyError>,
    > {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities
                .device_registry()
                .apply_events(sender_device_id, events)
        })
    }
}
