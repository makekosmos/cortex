use super::{
    DeviceId, DisplayName, RegistryEvent, RegistryEventError, RegistryOperation, SigningPublicKey,
};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    pub device_id: DeviceId,
    pub signing_public_key: SigningPublicKey,
    pub key_epoch: u64,
    pub admission_epoch: u64,
    pub display_name: Option<DisplayName>,
    pub disabled_event_id: Option<String>,
    pub display_name_hlc: Option<String>,
    pub updated_hlc: Option<String>,
    pub updated_event_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryHead {
    pub actor_device_id: DeviceId,
    pub actor_sequence: u64,
    pub event_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryApplyOutcome {
    Applied { event_hash: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryApplyError {
    #[error("registry actor is not enrolled")]
    UnknownActor,
    #[error("registry actor is disabled")]
    DisabledActor,
    #[error("registry actor admission epoch is stale")]
    StaleAdmission,
    #[error("registry event sequence {received} does not follow expected sequence {expected}")]
    SequenceGap { expected: u64, received: u64 },
    #[error("registry event does not continue the actor hash chain")]
    ChainFork,
    #[error("registry target is not enrolled")]
    UnknownTarget,
    #[error("registry target is already enrolled with a different identity")]
    ConflictingEnrollment,
    #[error("registry event key epoch is not the next epoch")]
    InvalidNextKeyEpoch,
    #[error(transparent)]
    InvalidEvent(#[from] RegistryEventError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceRegistry {
    devices: BTreeMap<DeviceId, DeviceRecord>,
    heads: BTreeMap<DeviceId, RegistryHead>,
}

impl DeviceRegistry {
    pub fn add_trust_anchor(&mut self, device: DeviceRecord) {
        self.devices.insert(device.device_id.clone(), device);
    }

    pub fn set_head(&mut self, head: RegistryHead) {
        self.heads.insert(head.actor_device_id.clone(), head);
    }

    pub fn device(&self, id: &DeviceId) -> Option<&DeviceRecord> {
        self.devices.get(id)
    }

    pub fn head(&self, id: &DeviceId) -> Option<&RegistryHead> {
        self.heads.get(id)
    }

    pub fn apply(
        &mut self,
        event: RegistryEvent,
    ) -> Result<RegistryApplyOutcome, RegistryApplyError> {
        let actor_id = event.actor_device_id().clone();
        let actor = self
            .devices
            .get(&actor_id)
            .ok_or(RegistryApplyError::UnknownActor)?;
        if actor.disabled_event_id.is_some() {
            return Err(RegistryApplyError::DisabledActor);
        }
        if event.actor_admission_epoch() != actor.admission_epoch {
            return Err(RegistryApplyError::StaleAdmission);
        }
        let verifying_key = actor.signing_public_key.verifying_key()?;
        let verified = event.verify(&actor_id, actor.key_epoch, &verifying_key)?;
        let expected_sequence = self
            .heads
            .get(&actor_id)
            .map(|head| head.actor_sequence.saturating_add(1))
            .unwrap_or(1);
        if event.actor_sequence() != expected_sequence {
            return Err(RegistryApplyError::SequenceGap {
                expected: expected_sequence,
                received: event.actor_sequence(),
            });
        }
        let expected_previous_hash = self
            .heads
            .get(&actor_id)
            .map(|head| head.event_hash.as_str());
        if event.previous_event_hash() != expected_previous_hash {
            return Err(RegistryApplyError::ChainFork);
        }

        let target_id = event.target_device_id().clone();
        match event.operation() {
            RegistryOperation::Enroll {
                signing_public_key,
                key_epoch,
                display_name,
            } => {
                if let Some(existing) = self.devices.get(&target_id) {
                    if existing.signing_public_key != *signing_public_key
                        || existing.key_epoch != *key_epoch
                    {
                        return Err(RegistryApplyError::ConflictingEnrollment);
                    }
                    let target = self
                        .devices
                        .get_mut(&target_id)
                        .ok_or(RegistryApplyError::UnknownTarget)?;
                    if let Some(display_name) = display_name {
                        let replace =
                            target
                                .display_name_hlc
                                .as_deref()
                                .is_none_or(|current_hlc| {
                                    match crate::hlc::HLC::compare_str(event.hlc(), current_hlc) {
                                        std::cmp::Ordering::Greater => true,
                                        std::cmp::Ordering::Less => false,
                                        std::cmp::Ordering::Equal => {
                                            target.display_name.as_ref().is_none_or(|current| {
                                                display_name.as_str() > current.as_str()
                                            })
                                        }
                                    }
                                });
                        if replace {
                            target.display_name = Some(display_name.clone());
                            target.display_name_hlc = Some(event.hlc().to_owned());
                        }
                    }
                } else {
                    self.devices.insert(
                        target_id.clone(),
                        DeviceRecord {
                            device_id: target_id.clone(),
                            signing_public_key: signing_public_key.clone(),
                            key_epoch: *key_epoch,
                            admission_epoch: 1,
                            display_name: display_name.clone(),
                            disabled_event_id: None,
                            display_name_hlc: display_name.as_ref().map(|_| event.hlc().to_owned()),
                            updated_hlc: None,
                            updated_event_id: None,
                        },
                    );
                }
            }
            RegistryOperation::Rename { display_name } => {
                let target = self
                    .devices
                    .get_mut(&target_id)
                    .ok_or(RegistryApplyError::UnknownTarget)?;
                let is_newer = target
                    .display_name_hlc
                    .as_deref()
                    .is_none_or(|current_hlc| {
                        match crate::hlc::HLC::compare_str(event.hlc(), current_hlc) {
                            std::cmp::Ordering::Greater => true,
                            std::cmp::Ordering::Less => false,
                            std::cmp::Ordering::Equal => target
                                .display_name
                                .as_ref()
                                .is_none_or(|current| display_name.as_str() > current.as_str()),
                        }
                    });
                if is_newer {
                    target.display_name = Some(display_name.clone());
                    target.display_name_hlc = Some(event.hlc().to_owned());
                }
            }
            RegistryOperation::Disable => {
                let target = self
                    .devices
                    .get_mut(&target_id)
                    .ok_or(RegistryApplyError::UnknownTarget)?;
                if target.disabled_event_id.is_none() {
                    target.admission_epoch = target.admission_epoch.saturating_add(1);
                    target.disabled_event_id = Some(event.event_id().to_owned());
                } else if target
                    .disabled_event_id
                    .as_ref()
                    .is_some_and(|current| event.event_id() < current.as_str())
                {
                    target.disabled_event_id = Some(event.event_id().to_owned());
                }
            }
            RegistryOperation::RebindKey {
                signing_public_key,
                key_epoch,
            } => {
                let target = self
                    .devices
                    .get_mut(&target_id)
                    .ok_or(RegistryApplyError::UnknownTarget)?;
                if *key_epoch != target.key_epoch.saturating_add(1) {
                    return Err(RegistryApplyError::InvalidNextKeyEpoch);
                }
                target.signing_public_key = signing_public_key.clone();
                target.key_epoch = *key_epoch;
                target.admission_epoch = target.admission_epoch.saturating_add(1);
            }
        }

        let target = self
            .devices
            .get_mut(&target_id)
            .ok_or(RegistryApplyError::UnknownTarget)?;
        if is_newer_event(
            event.hlc(),
            event.event_id(),
            target.updated_hlc.as_deref(),
            target.updated_event_id.as_deref(),
        ) {
            target.updated_hlc = Some(event.hlc().to_owned());
            target.updated_event_id = Some(event.event_id().to_owned());
        }
        let event_hash = verified.event_hash()?;
        let head = RegistryHead {
            actor_device_id: actor_id.clone(),
            actor_sequence: event.actor_sequence(),
            event_hash: event_hash.clone(),
        };
        self.heads.insert(actor_id, head);
        Ok(RegistryApplyOutcome::Applied { event_hash })
    }
}

fn is_newer_event(
    candidate_hlc: &str,
    candidate_id: &str,
    current_hlc: Option<&str>,
    current_id: Option<&str>,
) -> bool {
    match current_hlc {
        None => true,
        Some(current_hlc) => match crate::hlc::HLC::compare_str(candidate_hlc, current_hlc) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => {
                current_id.is_none_or(|current_id| candidate_id > current_id)
            }
        },
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
