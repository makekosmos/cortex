use super::super::executor::actor::{
    AppendLocalRegistryEventTask, EnrollPairedDeviceTask, SignRegistryHandshakeTask, WriterTask,
};
use super::*;
use ark_domain::device_registry::SigningPublicKey;
#[cfg(test)]
use ark_store::writer::SqliteWriter as StoreWriter;

impl DbHandle {
    pub(crate) async fn sign_registry_handshake(
        &self,
        context: CommandContext,
        signer_device_id: String,
        transcript: ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = serde_json::to_vec(&transcript).map_or(usize::MAX, |bytes| {
            signer_device_id.len().saturating_add(bytes.len())
        });
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        let candidate_secret_key_hex = ark_net::iroh::generate_secret_key_candidate();
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(SignRegistryHandshakeTask {
                    signer_device_id,
                    transcript,
                    candidate_secret_key_hex,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        StoreWriter::sign_registry_handshake_on_connection(
                            &mut connection,
                            &signer_device_id,
                            &transcript,
                            &candidate_secret_key_hex,
                        )
                        .map_err(executor::actor::map_registry_local_mutation_failure)
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn append_local_registry_event(
        &self,
        context: CommandContext,
        command: AppendLocalRegistryEventCommand,
    ) -> Result<Option<ark_domain::device_registry::RegistryEvent>, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = serde_json::to_vec(&command).map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        let (actor_device_id, command) = command.into_parts();
        let candidate_secret_key_hex = ark_net::iroh::generate_secret_key_candidate();
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(AppendLocalRegistryEventTask {
                    actor_device_id,
                    command,
                    candidate_secret_key_hex,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        StoreWriter::append_local_registry_event_on_connection(
                            &mut connection,
                            &actor_device_id,
                            command,
                            &candidate_secret_key_hex,
                        )
                        .map_err(executor::actor::map_registry_local_mutation_failure)
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }

    pub(crate) async fn enroll_paired_device(
        &self,
        context: CommandContext,
        command: EnrollPairedDeviceCommand,
    ) -> Result<ark_store::writer::DeviceEnrollmentOutcome, DbError> {
        if context.class != CommandClass::Interactive {
            return Err(DbError::Invalid("database command class mismatch".into()));
        }
        context.require_engine()?;
        let weight = serde_json::to_vec(&command).map_or(usize::MAX, |bytes| bytes.len());
        if weight == 0 || weight > super::super::MAX_COMMAND_BYTES {
            return Err(DbError::TooLarge);
        }
        let (local_device_id, target_device_id, transport_public_key) = command.into_parts();
        let endpoint_id: ark_net::iroh::EndpointId = transport_public_key
            .parse()
            .map_err(|_| DbError::Invalid("device_registry_invalid_endpoint_id".into()))?;
        let target_signing_key = SigningPublicKey::from_bytes(*endpoint_id.as_bytes())
            .map_err(|error| DbError::Invalid(error.to_string()))?;
        let candidate_secret_key_hex = ark_net::iroh::generate_secret_key_candidate();
        match &self.backend {
            HandleBackend::Actor(actor) => {
                let (reply, response) = tokio::sync::oneshot::channel();
                let task: Box<dyn WriterTask> = Box::new(EnrollPairedDeviceTask {
                    local_device_id,
                    target_device_id,
                    target_signing_key,
                    transport_public_key,
                    candidate_secret_key_hex,
                    reply,
                });
                actor.admit(context.class, &context.source, weight, task)?;
                response.await.unwrap_or(Err(DbError::OutcomeUnknown))
            }
            #[cfg(test)]
            HandleBackend::Test(connection) => {
                let admission = self
                    .admission
                    .try_acquire(context.class, &context.source, weight)
                    .map_err(map_admission_error)?;
                tokio::task::spawn_blocking({
                    let connection = Arc::clone(connection);
                    move || {
                        let _admission = admission;
                        let mut connection =
                            connection.lock().unwrap_or_else(|error| error.into_inner());
                        StoreWriter::enroll_paired_device_on_connection(
                            &mut connection,
                            &local_device_id,
                            &target_device_id,
                            &target_signing_key,
                            &transport_public_key,
                            &candidate_secret_key_hex,
                        )
                        .map_err(executor::actor::map_registry_local_mutation_failure)
                    }
                })
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
            }
        }
    }
}
