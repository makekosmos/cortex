use super::*;
use crate::domain::device_registry::{
    DeviceId, RegistryHandshakeTranscript, RegistryHandshakeTranscriptDraft,
};

#[tokio::test]
async fn production_storage_signs_and_verifies_only_active_registry_identities() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("device-registry-handshake.db")
        .to_string_lossy()
        .into_owned();
    let endpoint = iroh::SecretKey::generate().public().to_string();
    let (handle, mut owner) = DbHandle::open(db_path, "local-device".into())
        .await
        .unwrap();
    handle
        .submit(
            CommandContext::engine(),
            EnrollPairedDeviceCommand::new("local-device".into(), "paired-device".into(), endpoint),
        )
        .await
        .unwrap();

    let transcript = RegistryHandshakeTranscript::new(RegistryHandshakeTranscriptDraft {
        session_id: "session-1".into(),
        space_id: "space-1".into(),
        initiator_device_id: DeviceId::parse("local-device").unwrap(),
        responder_device_id: DeviceId::parse("paired-device").unwrap(),
        initiator_nonce: "01".repeat(32),
        responder_nonce: "02".repeat(32),
        protocol_version: 1,
        capabilities: vec!["registry_history_v1".into()],
    })
    .unwrap();
    let proof = handle
        .submit(
            CommandContext::engine(),
            SignRegistryHandshakeCommand::new("local-device".into(), transcript.clone()),
        )
        .await
        .unwrap();
    assert_eq!(proof.signer_device_id().as_str(), "local-device");
    handle
        .query(
            CommandContext::sync_system(CommandClass::SyncLive),
            VerifyRegistryHandshakeQuery::new("local-device".into(), transcript, proof),
        )
        .await
        .unwrap();

    owner
        .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
        .await
        .unwrap();
}
