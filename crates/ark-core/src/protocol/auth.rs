use sha2::{Digest, Sha256};

pub fn normalize_auth_secret(secret: Option<String>) -> Option<String> {
    secret
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn generate_auth_nonce() -> String {
    use rand::RngCore;

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

pub fn compute_hello_auth_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
) -> String {
    let message = format!("ark-sync-v1:hello:{space_id}:{device_id}:{nonce}");
    hmac_sha256_hex(secret.as_bytes(), message.as_bytes())
}

pub fn verify_hello_auth_hmac(
    secret: &str,
    space_id: &str,
    device_id: &str,
    nonce: &str,
    provided_hmac: &str,
) -> bool {
    let expected = compute_hello_auth_hmac(secret, space_id, device_id, nonce);
    constant_time_eq(expected.as_bytes(), provided_hmac.as_bytes())
}

fn hmac_sha256_hex(secret: &[u8], message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;

    let mut key = [0u8; BLOCK_SIZE];
    if secret.len() > BLOCK_SIZE {
        let digest = Sha256::digest(secret);
        key[..digest.len()].copy_from_slice(&digest);
    } else {
        key[..secret.len()].copy_from_slice(secret);
    }

    let mut outer_key_pad = [0x5c_u8; BLOCK_SIZE];
    let mut inner_key_pad = [0x36_u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        outer_key_pad[i] ^= key[i];
        inner_key_pad[i] ^= key[i];
    }

    let mut inner = Sha256::new();
    inner.update(inner_key_pad);
    inner.update(message);
    let inner_result = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_key_pad);
    outer.update(inner_result);
    hex_encode(&outer.finalize())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let max_len = a.len().max(b.len());
    for i in 0..max_len {
        let left = a.get(i).copied().unwrap_or(0);
        let right = b.get(i).copied().unwrap_or(0);
        diff |= (left ^ right) as usize;
    }
    diff == 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_hmac_helpers_are_deterministic_and_verify() {
        let nonce = "0123456789abcdef";
        let hmac = compute_hello_auth_hmac("secret", "space-a", "device-a", nonce);

        assert_eq!(hmac.len(), 64);
        assert!(verify_hello_auth_hmac(
            "secret", "space-a", "device-a", nonce, &hmac
        ));
        assert!(!verify_hello_auth_hmac(
            "wrong", "space-a", "device-a", nonce, &hmac
        ));
        assert!(!verify_hello_auth_hmac(
            "secret", "space-a", "device-b", nonce, &hmac
        ));
    }

    #[test]
    fn auth_nonce_is_random_hex_64() {
        let nonce_a = generate_auth_nonce();
        let nonce_b = generate_auth_nonce();

        assert_eq!(nonce_a.len(), 64);
        assert!(nonce_a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(nonce_a, nonce_b);
    }
}
