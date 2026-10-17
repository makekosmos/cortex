use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use ark_domain::device_registry::{
    state::DeviceRegistry, DeviceId, RegistryHandshakeProof, RegistryHandshakeTranscript,
};
use rusqlite::{Connection, Transaction};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignRegistryHandshakeCommand {
    signer_device_id: String,
    transcript: RegistryHandshakeTranscript,
}

impl SignRegistryHandshakeCommand {
    pub(crate) fn new(signer_device_id: String, transcript: RegistryHandshakeTranscript) -> Self {
        Self {
            signer_device_id,
            transcript,
        }
    }
}

impl sealed::Sealed for SignRegistryHandshakeCommand {}

impl DbCommand for SignRegistryHandshakeCommand {
    type Output = RegistryHandshakeProof;
    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let signer = DeviceId::parse(self.signer_device_id)
            .map_err(|error| DbError::Invalid(error.to_string()))?;
        let registry = super::device_registry_sync::load_registry(transaction)?;
        let device = known_registry_device(&registry, &signer)?;
        let secret = crate::iroh_transport::load_or_generate_secret_key(transaction)
            .map_err(DbError::Storage)?;
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret.to_bytes());
        let local_public_key =
            ark_domain::device_registry::SigningPublicKey::from_bytes(*secret.public().as_bytes())
                .map_err(|error| DbError::Invalid(error.to_string()))?;
        if device.signing_public_key != local_public_key {
            return Err(DbError::Conflict(
                "device_registry_local_identity_conflict".into(),
            ));
        }
        self.transcript
            .sign(signer, &signing_key)
            .map_err(|error| DbError::Invalid(error.to_string()))
    }
}

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

impl DbQuery for VerifyRegistryHandshakeQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        let signer = DeviceId::parse(self.expected_signer)
            .map_err(|error| DbError::Invalid(error.to_string()))?;
        let registry = super::device_registry_sync::load_registry(connection)?;
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
