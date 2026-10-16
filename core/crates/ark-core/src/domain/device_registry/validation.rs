use super::encoding::is_lower_hex;
use super::{
    DeviceId, DisplayName, RegistryEvent, RegistryEventDraft, RegistryEventError,
    RegistryOperation, EVENT_SCHEMA_VERSION,
};

pub(super) fn validate_draft(draft: &RegistryEventDraft) -> Result<(), RegistryEventError> {
    if draft.actor_sequence == 0 || draft.actor_key_epoch == 0 || draft.actor_admission_epoch == 0 {
        return Err(RegistryEventError::InvalidField {
            field: "sequence_epoch",
        });
    }
    match (draft.actor_sequence, &draft.previous_event_hash) {
        (1, None) => {}
        (1, Some(_)) | (_, None) => {
            return Err(RegistryEventError::InvalidField {
                field: "previous_event_hash",
            });
        }
        (_, Some(hash)) if !is_lower_hex(hash, 64) => {
            return Err(RegistryEventError::InvalidField {
                field: "previous_event_hash",
            });
        }
        _ => {}
    }
    validate_operation(
        &draft.actor_device_id,
        &draft.target_device_id,
        &draft.hlc,
        &draft.operation,
    )
}

pub(super) fn validate_event(event: &RegistryEvent) -> Result<(), RegistryEventError> {
    if event.schema_version != EVENT_SCHEMA_VERSION
        || event.event_id
            != format!(
                "{}:{}",
                event.actor_device_id.as_str(),
                event.actor_sequence
            )
    {
        return Err(RegistryEventError::InvalidField { field: "event_id" });
    }
    validate_draft(&RegistryEventDraft {
        actor_device_id: event.actor_device_id.clone(),
        actor_sequence: event.actor_sequence,
        previous_event_hash: event.previous_event_hash.clone(),
        target_device_id: event.target_device_id.clone(),
        actor_key_epoch: event.actor_key_epoch,
        actor_admission_epoch: event.actor_admission_epoch,
        hlc: event.hlc.clone(),
        operation: event.operation.clone(),
    })
}

fn validate_operation(
    actor: &DeviceId,
    target: &DeviceId,
    hlc: &str,
    operation: &RegistryOperation,
) -> Result<(), RegistryEventError> {
    let parsed_hlc = crate::hlc::HLC::from_string(hlc);
    if hlc.is_empty()
        || hlc.len() > 256
        || hlc.chars().any(char::is_control)
        || parsed_hlc.to_string() != hlc
        || parsed_hlc.device_id != actor.as_str()
    {
        return Err(RegistryEventError::InvalidField { field: "hlc" });
    }
    match operation {
        RegistryOperation::Enroll { key_epoch, .. }
        | RegistryOperation::RebindKey { key_epoch, .. }
            if *key_epoch == 0 =>
        {
            Err(RegistryEventError::InvalidField { field: "key_epoch" })
        }
        RegistryOperation::Rename { display_name } => {
            DisplayName::parse(display_name.as_str()).map(|_| ())
        }
        RegistryOperation::Disable if actor == target => Err(RegistryEventError::InvalidField {
            field: "self_disable",
        }),
        RegistryOperation::Enroll {
            display_name,
            key_epoch,
            ..
        } => {
            if actor == target || *key_epoch != 1 {
                return Err(RegistryEventError::InvalidField {
                    field: "enrollment",
                });
            }
            DisplayName::parse(display_name.as_str()).map(|_| ())
        }
        RegistryOperation::RebindKey { .. } | RegistryOperation::Disable => Ok(()),
    }
}
