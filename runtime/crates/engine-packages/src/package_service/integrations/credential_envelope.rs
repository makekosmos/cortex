use super::secret_store::{read_package_integration_secret, save_package_integration_secret};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hpke::{
    aead::ChaCha20Poly1305,
    kdf::HkdfSha256,
    kem::{Kem, X25519HkdfSha256},
    setup_receiver, setup_sender, Deserializable, OpModeR, OpModeS, Serializable,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
/// Fixture builds keep package secrets in the in-memory test store, not the OS
/// keyring; integration tests read the replicated secret back through this.
#[cfg(feature = "package-worker-fixture")]
pub fn fixture_read_integration_secret(id: &str, version: &str, setting: &str) -> Option<String> {
    read_package_integration_secret(id, version, setting)
}

pub const ENVELOPE_VERSION: u8 = 2;
const ALGORITHM: &str = "HPKE-Auth-X25519-HKDF-SHA256-ChaCha20Poly1305";
const HPKE_INFO: &[u8] = b"makekosmos/cortex/credential-envelope/v2";
const IDENTITY_PACKAGE: &str = "__cortex__";
const IDENTITY_VERSION: &str = "credential-envelope-v2";

#[path = "credential_envelope_models.rs"]
mod models;
pub use models::{CredentialContext, CredentialEnvelopeV2, DecryptedSecret};
pub use models::HpkeIdentity;

#[path = "credential_envelope_types.rs"]
mod types;
pub use types::CredentialEnvelopeError;
#[path = "credential_identity_cleanup.rs"]
mod identity_cleanup;
pub use identity_cleanup::clear_identity;


pub fn load_or_create_identity(node_id: &str) -> Result<HpkeIdentity, CredentialEnvelopeError> {
    validate_id(node_id)?;
    let setting = identity_setting(node_id);
    if let Some(private_key) = read_package_integration_secret(
        IDENTITY_PACKAGE,
        IDENTITY_VERSION,
        &setting,
    ) {
        return identity_from_private(node_id, &private_key);
    }

    let (private_key, public_key) = X25519HkdfSha256::gen_keypair();
    let private_key = encode(private_key.to_bytes().as_ref());
    let public_key = encode(public_key.to_bytes().as_ref());
    save_package_integration_secret(IDENTITY_PACKAGE, IDENTITY_VERSION, &setting, &private_key)?;
    Ok(HpkeIdentity {
        node_id: node_id.to_owned(),
        key_id: encryption_key_id(&public_key),
        private_key,
        public_key,
    })
}

pub fn encrypt(
    context: &CredentialContext,
    recipient_public_key: &str,
    sender: &HpkeIdentity,
    plaintext: &str,
) -> Result<CredentialEnvelopeV2, CredentialEnvelopeError> {
    validate_context(context)?;
    if sender.node_id != context.issuer_node_id {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    let recipient_public_key = decode_key(recipient_public_key)?;
    if context.key_id != encryption_key_id(&encode(recipient_public_key.to_bytes().as_ref())) {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    let sender_private_key = decode_private_key(&sender.private_key)?;
    let sender_public_key = X25519HkdfSha256::sk_to_pk(&sender_private_key);
    if sender.public_key != encode(sender_public_key.to_bytes().as_ref())
        || sender.key_id != encryption_key_id(&sender.public_key)
    {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }

    let (enc, mut context_hpke) = setup_sender::<ChaCha20Poly1305, HkdfSha256, X25519HkdfSha256>(
        &OpModeS::Auth((sender_private_key, sender_public_key)),
        &recipient_public_key,
        HPKE_INFO,
    )
    .map_err(|_| CredentialEnvelopeError::Hpke)?;
    let envelope = CredentialEnvelopeV2 {
        version: ENVELOPE_VERSION,
        algorithm: ALGORITHM.to_owned(),
        integration_id: context.integration_id.clone(),
        recipient_node_id: context.recipient_node_id.clone(),
        grant_epoch: context.grant_epoch,
        credential_generation: context.credential_generation,
        key_id: context.key_id.clone(),
        issuer_node_id: context.issuer_node_id.clone(),
        issuer_auth_key_id: context.issuer_auth_key_id.clone(),
        enc: encode(enc.to_bytes().as_ref()),
        ciphertext: String::new(),
    };
    let aad = aad(&envelope)?;
    let ciphertext = context_hpke
        .seal(plaintext.as_bytes(), &aad)
        .map_err(|_| CredentialEnvelopeError::Hpke)?;
    Ok(CredentialEnvelopeV2 {
        ciphertext: encode(&ciphertext),
        ..envelope
    })
}

pub fn decrypt(
    envelope: &CredentialEnvelopeV2,
    expected: &CredentialContext,
    recipient: &HpkeIdentity,
    issuer_public_key: &str,
) -> Result<DecryptedSecret, CredentialEnvelopeError> {
    validate_envelope(envelope, expected, recipient)?;
    let sender_public_key = decode_key(issuer_public_key)?;
    if encryption_key_id(&encode(sender_public_key.to_bytes().as_ref()))
        != expected.issuer_auth_key_id
    {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    let enc = <X25519HkdfSha256 as Kem>::EncappedKey::from_bytes(&decode(&envelope.enc)?)
        .map_err(|_| CredentialEnvelopeError::InvalidEncoding)?;
    let ciphertext = decode(&envelope.ciphertext)?;
    let recipient_private_key = decode_private_key(&recipient.private_key)?;
    let mut context_hpke = setup_receiver::<ChaCha20Poly1305, HkdfSha256, X25519HkdfSha256>(
        &OpModeR::Auth(sender_public_key),
        &recipient_private_key,
        &enc,
        HPKE_INFO,
    )
    .map_err(|_| CredentialEnvelopeError::Authentication)?;
    let plaintext = context_hpke
        .open(&ciphertext, &aad(envelope)?)
        .map_err(|_| CredentialEnvelopeError::Authentication)?;
    String::from_utf8(plaintext)
        .map(DecryptedSecret)
        .map_err(|error| {
            let mut bytes = error.into_bytes();
            for byte in &mut bytes { unsafe { std::ptr::write_volatile(byte, 0) }; }
            CredentialEnvelopeError::InvalidPlaintext
        })
}

pub fn decrypt_and_store(
    envelope: &CredentialEnvelopeV2,
    expected: &CredentialContext,
    recipient: &HpkeIdentity,
    issuer_public_key: &str,
    package_id: &str,
    package_version: &str,
    setting: &str,
) -> Result<(), CredentialEnvelopeError> {
    if package_id != expected.integration_id {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    let plaintext = decrypt(envelope, expected, recipient, issuer_public_key)?;
    save_package_integration_secret(package_id, package_version, setting, plaintext.as_str())?;
    Ok(())
}

fn validate_context(context: &CredentialContext) -> Result<(), CredentialEnvelopeError> {
    for value in [
        &context.integration_id,
        &context.recipient_node_id,
        &context.key_id,
        &context.issuer_node_id,
    ] {
        validate_id(value)?;
    }
    if context.grant_epoch == 0 || context.credential_generation == 0 {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    Ok(())
}

fn validate_envelope(
    envelope: &CredentialEnvelopeV2,
    expected: &CredentialContext,
    recipient: &HpkeIdentity,
) -> Result<(), CredentialEnvelopeError> {
    validate_context(expected)?;
    if recipient.node_id != expected.recipient_node_id
        || recipient.key_id != expected.key_id
        || envelope.version != ENVELOPE_VERSION
        || envelope.algorithm != ALGORITHM
        || envelope.integration_id != expected.integration_id
        || envelope.recipient_node_id != expected.recipient_node_id
        || envelope.grant_epoch != expected.grant_epoch
        || envelope.credential_generation != expected.credential_generation
        || envelope.key_id != expected.key_id
        || envelope.issuer_node_id != expected.issuer_node_id
        || envelope.issuer_auth_key_id != expected.issuer_auth_key_id
        || envelope.enc.is_empty()
        || envelope.ciphertext.is_empty()
    {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    Ok(())
}

#[derive(Serialize)]
struct Aad<'a> {
    version: u8,
    algorithm: &'a str,
    integration_id: &'a str,
    recipient_node_id: &'a str,
    grant_epoch: u64,
    credential_generation: u64,
    key_id: &'a str,
    issuer_node_id: &'a str,
    issuer_auth_key_id: &'a str,
}

fn aad(envelope: &CredentialEnvelopeV2) -> Result<Vec<u8>, CredentialEnvelopeError> {
    serde_json::to_vec(&Aad {
        version: envelope.version,
        algorithm: &envelope.algorithm,
        integration_id: &envelope.integration_id,
        recipient_node_id: &envelope.recipient_node_id,
        grant_epoch: envelope.grant_epoch,
        credential_generation: envelope.credential_generation,
        key_id: &envelope.key_id,
        issuer_node_id: &envelope.issuer_node_id,
        issuer_auth_key_id: &envelope.issuer_auth_key_id,
    })
    .map_err(|_| CredentialEnvelopeError::InvalidEnvelope)
}

fn identity_from_private(
    node_id: &str,
    private_key: &str,
) -> Result<HpkeIdentity, CredentialEnvelopeError> {
    let private_key_typed = decode_private_key(private_key)?;
    let public_key = encode(X25519HkdfSha256::sk_to_pk(&private_key_typed).to_bytes().as_ref());
    Ok(HpkeIdentity {
        node_id: node_id.to_owned(),
        key_id: encryption_key_id(&public_key),
        private_key: private_key.to_owned(),
        public_key,
    })
}

fn decode_private_key(value: &str) -> Result<X25519HkdfSha256PrivateKey, CredentialEnvelopeError> {
    X25519HkdfSha256PrivateKey::from_bytes(&decode(value)?)
        .map_err(|_| CredentialEnvelopeError::InvalidEncoding)
}

type X25519HkdfSha256PrivateKey = <X25519HkdfSha256 as Kem>::PrivateKey;

fn decode_key(
    value: &str
) -> Result<<X25519HkdfSha256 as Kem>::PublicKey, CredentialEnvelopeError> {
    <X25519HkdfSha256 as Kem>::PublicKey::from_bytes(&decode(value)?)
        .map_err(|_| CredentialEnvelopeError::InvalidEncoding)
}

fn decode(value: &str) -> Result<Vec<u8>, CredentialEnvelopeError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| CredentialEnvelopeError::InvalidEncoding)?;
    if value.is_empty() || URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err(CredentialEnvelopeError::InvalidEncoding);
    }
    Ok(bytes)
}

fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn encryption_key_id(public_key: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(public_key.as_bytes()))
}

fn identity_setting(node_id: &str) -> String {
    format!("hpke-auth:{node_id}")
}

fn validate_id(value: &str) -> Result<(), CredentialEnvelopeError> {
    if value.is_empty()
        || value.len() > 256
        || value.bytes().any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(CredentialEnvelopeError::InvalidEnvelope);
    }
    Ok(())
}

#[cfg(test)]
#[path = "credential_envelope_tests.rs"]
mod tests;
