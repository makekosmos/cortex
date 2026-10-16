use ed25519_dalek::Signature;

use super::{RegistryEventError, SigningPayload};

pub(super) fn canonical_bytes(payload: &SigningPayload<'_>) -> Result<Vec<u8>, RegistryEventError> {
    let mut bytes = b"cortex-device-registry-event-v1\0".to_vec();
    bytes.extend(
        serde_json::to_vec(payload).map_err(|_| RegistryEventError::InvalidField {
            field: "canonical_payload",
        })?,
    );
    Ok(bytes)
}

pub(super) fn decode_signature(value: &str) -> Result<Signature, RegistryEventError> {
    if !is_lower_hex(value, 128) {
        return Err(RegistryEventError::InvalidField { field: "signature" });
    }
    let mut bytes = [0; 64];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| RegistryEventError::InvalidField { field: "signature" })?;
    }
    Ok(Signature::from_bytes(&bytes))
}

pub(super) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

pub(super) fn is_lower_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_hex_parser_requires_exact_lowercase_length() {
        assert!(is_lower_hex("01ab", 4));
        assert!(!is_lower_hex("01AB", 4));
        assert!(!is_lower_hex("01a", 4));
    }
}
