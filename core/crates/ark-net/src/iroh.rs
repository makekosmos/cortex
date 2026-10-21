pub use iroh::{EndpointAddr, EndpointId, RelayMode, RelayUrl, SecretKey};

pub trait IrohIdentityStore {
    fn load_secret_key(&self) -> Result<Option<String>, String>;
    fn insert_secret_key_if_absent(&self, candidate_hex: &str) -> Result<String, String>;
}

pub fn generate_secret_key_candidate() -> String {
    hex_encode(&SecretKey::generate().to_bytes())
}

pub fn parse_secret_key_hex(persisted: &str) -> Result<SecretKey, String> {
    secret_key_from_hex(persisted)
}

pub fn load_or_create_secret_key(store: &dyn IrohIdentityStore) -> Result<SecretKey, String> {
    let Some(persisted) = store.load_secret_key()? else {
        return parse_secret_key_hex(
            &store.insert_secret_key_if_absent(&generate_secret_key_candidate())?,
        );
    };
    parse_secret_key_hex(&persisted)
}

fn secret_key_from_hex(persisted: &str) -> Result<SecretKey, String> {
    let bytes = hex_decode_32(persisted)
        .ok_or_else(|| "iroh transport: stored secret key is not valid hex".to_string())?;
    Ok(SecretKey::from_bytes(&bytes))
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode_32(value: &str) -> Option<[u8; 32]> {
    let value = value.trim();
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = (value.as_bytes()[index * 2] as char).to_digit(16)?;
        let low = (value.as_bytes()[index * 2 + 1] as char).to_digit(16)?;
        *byte = ((high << 4) | low) as u8;
    }
    Some(bytes)
}

#[cfg(test)]
mod identity_tests {
    use std::sync::Mutex;

    use super::{load_or_create_secret_key, IrohIdentityStore};

    #[derive(Default)]
    struct MemoryIdentityStore(Mutex<Option<String>>);

    impl IrohIdentityStore for MemoryIdentityStore {
        fn load_secret_key(&self) -> Result<Option<String>, String> {
            Ok(self.0.lock().expect("identity store lock").clone())
        }

        fn insert_secret_key_if_absent(&self, candidate_hex: &str) -> Result<String, String> {
            let mut stored = self.0.lock().expect("identity store lock");
            Ok(stored
                .get_or_insert_with(|| candidate_hex.to_owned())
                .clone())
        }
    }

    #[test]
    fn generated_identity_is_reused_from_storage() {
        let store = MemoryIdentityStore::default();
        let first = load_or_create_secret_key(&store).expect("identity generates");
        let second = load_or_create_secret_key(&store).expect("identity reloads");
        assert_eq!(first.public(), second.public());
    }

    #[test]
    fn invalid_persisted_identity_fails_closed() {
        let store = MemoryIdentityStore(Mutex::new(Some("invalid".to_owned())));
        let error = load_or_create_secret_key(&store).expect_err("invalid key is rejected");
        assert!(error.contains("stored secret key is not valid hex"));
    }
}

include!("iroh_imports.rs");
include!("iroh_config.rs");
include!("iroh_registry.rs");
include!("iroh_transport_impl.rs");
include!("iroh_connections.rs");
include!("iroh_relay_watch.rs");
include!("iroh_sync_impl.rs");
include!("iroh_peer_session_control.rs");
include!("iroh_message_helpers.rs");
include!("iroh_write_frame.rs");
include!("iroh_read_frame.rs");
include!("iroh_tail.rs");
