use rusqlite::Connection;

const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

pub fn load_or_generate_secret_key(conn: &Connection) -> Result<ark_net::iroh::SecretKey, String> {
    if let Some(stored) = ark_store::sync_kv::get_sync_kv(conn, IROH_SECRET_KEY_KV_KEY)? {
        let bytes = hex_decode_32(&stored)
            .ok_or_else(|| "iroh transport: stored secret key is not valid hex".to_string())?;
        return Ok(ark_net::iroh::SecretKey::from_bytes(&bytes));
    }

    let secret_key = ark_net::iroh::SecretKey::generate();
    ark_store::sync_kv::set_sync_kv(
        conn,
        IROH_SECRET_KEY_KV_KEY,
        &hex_encode(&secret_key.to_bytes()),
    )?;
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
    for (index, byte) in out.iter_mut().enumerate() {
        let high = (s.as_bytes()[index * 2] as char).to_digit(16)?;
        let low = (s.as_bytes()[index * 2 + 1] as char).to_digit(16)?;
        *byte = ((high << 4) | low) as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::load_or_generate_secret_key;
    use super::IROH_SECRET_KEY_KV_KEY;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn secret_key_is_persisted_across_calls_on_same_storage() {
        let conn = setup_db();
        let key_a = load_or_generate_secret_key(&conn).expect("first load/generate");
        let key_b = load_or_generate_secret_key(&conn).expect("second load/generate");
        assert_eq!(key_a.public(), key_b.public());
    }

    #[test]
    fn secret_key_differs_across_independent_storages() {
        let conn_1 = setup_db();
        let conn_2 = setup_db();
        let key_1 = load_or_generate_secret_key(&conn_1).expect("storage 1 load/generate");
        let key_2 = load_or_generate_secret_key(&conn_2).expect("storage 2 load/generate");
        assert_ne!(key_1.public(), key_2.public());
    }

    #[test]
    fn secret_key_is_written_to_sync_kv_after_first_call() {
        let conn = setup_db();
        assert_eq!(
            ark_store::sync_kv::get_sync_kv(&conn, IROH_SECRET_KEY_KV_KEY).unwrap(),
            None
        );
        let _ = load_or_generate_secret_key(&conn).expect("first load/generate");
        assert!(
            ark_store::sync_kv::get_sync_kv(&conn, IROH_SECRET_KEY_KV_KEY)
                .unwrap()
                .is_some()
        );
    }
}
