// Identity — Фаза 1 шаг 1: постоянный iroh `SecretKey` на устройство.
// ---------------------------------------------------------------------------

/// Загружает persisted iroh `SecretKey` из `sync_kv` (ключ
/// `iroh.secret_key`) либо генерирует новый и сохраняет его туда же.
///
/// Это даёт стабильный `EndpointId` между рестартами процесса.
///
/// Принимает `&Connection` напрямую (как `get_sync_kv`/`set_sync_kv` в
/// `db.rs`), а не `Arc<dyn StorageBackend>`.
pub fn load_or_generate_secret_key(conn: &Connection) -> Result<iroh::SecretKey, String> {
    if let Some(stored) = get_sync_kv(conn, IROH_SECRET_KEY_KV_KEY)? {
        let bytes = hex_decode_32(&stored)
            .ok_or_else(|| "iroh transport: stored secret key is not valid hex".to_string())?;
        return Ok(iroh::SecretKey::from_bytes(&bytes));
    }

    let secret_key = iroh::SecretKey::generate();
    let encoded = hex_encode(&secret_key.to_bytes());
    set_sync_kv(conn, IROH_SECRET_KEY_KV_KEY, &encoded)?;
    Ok(secret_key)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode_32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    for i in 0..32 {
        let hi = (bytes[i * 2] as char).to_digit(16)?;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// Tests — unit-тесты (ticket codec, registry, identity).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod ticket_tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use iroh::{EndpointAddr, SecretKey};

    use super::{from_ticket, IrohTransport};

    fn sample_addr() -> EndpointAddr {
        let secret = SecretKey::generate();
        let socket: SocketAddr = (Ipv4Addr::LOCALHOST, 4242).into();
        EndpointAddr::new(secret.public()).with_ip_addr(socket)
    }

    #[test]
    fn ticket_round_trips_through_string() {
        let addr = sample_addr();
        let expected_id = addr.id;

        let ticket_str = IrohTransport::ticket_string_for_addr(&addr);
        let parsed = from_ticket(&ticket_str).expect("ticket should parse back");

        assert_eq!(
            parsed.id, expected_id,
            "round-tripped ticket should preserve EndpointId"
        );
    }

    #[test]
    fn from_ticket_rejects_garbage_string() {
        let result = from_ticket("not-a-real-ticket");
        assert!(result.is_err(), "garbage string must not parse as a ticket");
    }
}

#[cfg(test)]
mod registry_tests {
    use iroh::SecretKey;

    use super::DeviceRegistry;

    #[test]
    fn resolves_device_id_by_endpoint_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A".to_string())
        );
    }

    #[test]
    fn resolves_endpoint_id_by_device_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(registry.endpoint_id_for("device-A"), Some(endpoint_id));
    }

    #[test]
    fn replacing_endpoint_clears_old_trust() {
        let registry = DeviceRegistry::new();
        let old = SecretKey::generate().public();
        let new = SecretKey::generate().public();
        registry.insert(old, "device-A".to_string());
        assert!(registry.bind_authenticated("device-A", &old.to_string()));
        registry.insert(new, "device-A".to_string());
        assert!(registry.authenticated_endpoint("device-A").is_none());
    }

    #[test]
    fn unknown_endpoint_id_resolves_to_none() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        assert_eq!(registry.device_id_for(&endpoint_id), None);
    }

    #[test]
    fn later_insert_overwrites_earlier_mapping_for_same_endpoint_id() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());
        registry.insert(endpoint_id, "device-A-renamed".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A-renamed".to_string())
        );
        assert_eq!(registry.endpoint_id_for("device-A"), None);
    }

    #[test]
    fn untrusted_hello_cannot_replace_trusted_endpoint_mapping() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());
        assert!(registry.bind_authenticated("device-A", &endpoint_id.to_string()));

        assert!(!registry.insert_untrusted(endpoint_id, "device-attacker".to_string()));
        assert_eq!(registry.device_id_for(&endpoint_id).as_deref(), Some("device-A"));
        assert_eq!(registry.authenticated_endpoint("device-A"), Some(endpoint_id));
    }

    #[test]
    fn untrusted_hello_cannot_move_trusted_device_to_new_endpoint() {
        let registry = DeviceRegistry::new();
        let old_endpoint = SecretKey::generate().public();
        let new_endpoint = SecretKey::generate().public();

        registry.insert(old_endpoint, "device-A".to_string());
        assert!(registry.bind_authenticated("device-A", &old_endpoint.to_string()));

        assert!(!registry.insert_untrusted(new_endpoint, "device-A".to_string()));
        assert_eq!(registry.endpoint_id_for("device-A"), Some(old_endpoint));
        assert_eq!(registry.authenticated_endpoint("device-A"), Some(old_endpoint));
    }

    #[test]
    fn inserting_existing_device_removes_its_old_endpoint_mapping() {
        let registry = DeviceRegistry::new();
        let old_endpoint = SecretKey::generate().public();
        let new_endpoint = SecretKey::generate().public();

        registry.insert(old_endpoint, "device-A".to_string());
        registry.insert(new_endpoint, "device-A".to_string());

        assert_eq!(registry.device_id_for(&old_endpoint), None);
        assert_eq!(registry.endpoint_id_for("device-A"), Some(new_endpoint));
    }
}

#[cfg(test)]
mod identity_tests {
    use rusqlite::Connection;

    use crate::db::init_schema;

    use super::load_or_generate_secret_key;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn secret_key_is_persisted_across_calls_on_same_storage() {
        let conn = setup_db();

        let key_a = load_or_generate_secret_key(&conn).expect("first load/generate");
        let key_b = load_or_generate_secret_key(&conn).expect("second load/generate");

        assert_eq!(
            key_a.public(),
            key_b.public(),
            "EndpointId должен быть стабилен между вызовами на одном storage"
        );
    }

    #[test]
    fn secret_key_differs_across_independent_storages() {
        let conn_1 = setup_db();
        let conn_2 = setup_db();

        let key_1 = load_or_generate_secret_key(&conn_1).expect("storage 1 load/generate");
        let key_2 = load_or_generate_secret_key(&conn_2).expect("storage 2 load/generate");

        assert_ne!(
            key_1.public(),
            key_2.public(),
            "независимые storage должны получать разные identity"
        );
    }

    #[test]
    fn secret_key_is_written_to_sync_kv_after_first_call() {
        let conn = setup_db();

        assert_eq!(
            crate::db::get_sync_kv(&conn, "iroh.secret_key").unwrap(),
            None,
            "до первого вызова ключа в sync_kv быть не должно"
        );

        let _ = load_or_generate_secret_key(&conn).expect("first load/generate");

        let stored = crate::db::get_sync_kv(&conn, "iroh.secret_key").unwrap();
        assert!(
            stored.is_some(),
            "после первого вызова secret key должен быть записан в sync_kv"
        );
    }
}
