use super::task::{settle, Publish, WriterTask};
use crate::db::DbError;
use ark_domain::device_registry::{
    RegistryCommand, RegistryEvent, RegistryHandshakeProof, RegistryHandshakeTranscript,
    SigningPublicKey,
};
use ark_store::writer::{
    DeviceEnrollmentOutcome, RegistryLocalMutationError, SqliteWriter, StoreTransactionFailure,
};
use tokio::sync::oneshot;

pub(in crate::db::command) struct AppendLocalRegistryEventTask {
    pub actor_device_id: String,
    pub command: RegistryCommand,
    pub candidate_secret_key_hex: String,
    pub reply: oneshot::Sender<Result<Option<RegistryEvent>, DbError>>,
}

impl WriterTask for AppendLocalRegistryEventTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            actor_device_id,
            command,
            candidate_secret_key_hex,
            reply,
        } = *self;
        let result = writer
            .append_local_registry_event(&actor_device_id, command, &candidate_secret_key_hex)
            .map_err(map_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }
    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct EnrollPairedDeviceTask {
    pub local_device_id: String,
    pub target_device_id: String,
    pub target_signing_key: SigningPublicKey,
    pub transport_public_key: String,
    pub candidate_secret_key_hex: String,
    pub reply: oneshot::Sender<Result<DeviceEnrollmentOutcome, DbError>>,
}

pub(in crate::db::command) struct SignRegistryHandshakeTask {
    pub signer_device_id: String,
    pub transcript: RegistryHandshakeTranscript,
    pub candidate_secret_key_hex: String,
    pub reply: oneshot::Sender<Result<RegistryHandshakeProof, DbError>>,
}

impl WriterTask for SignRegistryHandshakeTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            signer_device_id,
            transcript,
            candidate_secret_key_hex,
            reply,
        } = *self;
        let result = writer
            .sign_registry_handshake(&signer_device_id, &transcript, &candidate_secret_key_hex)
            .map_err(map_failure);
        settle(result, reply, on_error)
    }
    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }
    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

impl WriterTask for EnrollPairedDeviceTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            local_device_id,
            target_device_id,
            target_signing_key,
            transport_public_key,
            candidate_secret_key_hex,
            reply,
        } = *self;
        let result = writer
            .enroll_paired_device(
                &local_device_id,
                &target_device_id,
                &target_signing_key,
                &transport_public_key,
                &candidate_secret_key_hex,
            )
            .map_err(map_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }
    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) fn map_failure(
    failure: StoreTransactionFailure<RegistryLocalMutationError>,
) -> DbError {
    match failure {
        StoreTransactionFailure::Begin(error) => DbError::Storage(error.to_string()),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
        StoreTransactionFailure::Operation(error) => map_operation_error(error),
    }
}

pub(in crate::db::command) fn map_operation_error(error: RegistryLocalMutationError) -> DbError {
    match error {
        RegistryLocalMutationError::Invalid(message) => DbError::Invalid(message),
        RegistryLocalMutationError::Conflict(message) => DbError::Conflict(message),
        RegistryLocalMutationError::Permission(message) => DbError::Permission(message),
        RegistryLocalMutationError::Storage(message) => DbError::Storage(message),
    }
}
