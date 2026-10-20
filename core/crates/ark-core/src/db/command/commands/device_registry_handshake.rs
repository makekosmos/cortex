use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use ark_domain::device_registry::{
    state::DeviceRegistry, DeviceId, RegistryHandshakeProof, RegistryHandshakeTranscript,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct VerifyRegistryHandshakeQuery {
    expected_signer: String,
    transcript: RegistryHandshakeTranscript,
    proof: RegistryHandshakeProof,
}

impl VerifyRegistryHandshakeQuery {
    pub(crate) fn new(
        expected_signer: String,
        transcript: RegistryHandshakeTranscript,
        proof: RegistryHandshakeProof,
    ) -> Self {
        Self {
            expected_signer,
            transcript,
            proof,
        }
    }
}

impl sealed::Sealed for VerifyRegistryHandshakeQuery {}

impl StoreReadQuery for VerifyRegistryHandshakeQuery {
    type Output = ();

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        let signer = DeviceId::parse(self.expected_signer)
            .map_err(|error| DbError::Invalid(error.to_string()))?;
        let registry = capabilities
            .device_registry()
            .registry()
            .map_err(DbError::Storage)?;
        let device = known_registry_device(&registry, &signer)?;
        let verifying_key = device
            .signing_public_key
            .verifying_key()
            .map_err(|error| DbError::Invalid(error.to_string()))?;
        self.proof
            .verify(&self.transcript, &signer, &verifying_key)
            .map_err(|error| DbError::Permission(error.to_string()))
    }
}

fn known_registry_device<'a>(
    registry: &'a DeviceRegistry,
    device_id: &DeviceId,
) -> Result<&'a ark_domain::device_registry::DeviceRecord, DbError> {
    registry
        .device(device_id)
        .ok_or_else(|| DbError::Permission("device_registry_peer_unknown".into()))
}
