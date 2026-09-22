use serde_json::Value;

use super::{IntegrationReplicationChange, SignedSyncError};

const DOMAIN: &[u8] = b"kosmos.ark.signed-sync.v1";

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(super) fn canonical_signing_bytes(
    space_id: &str,
    origin_node_id: &str,
    recipient_node_id: &str,
    key_epoch: u64,
    message_id: &str,
    payload: &[IntegrationReplicationChange],
) -> Result<Vec<u8>, SignedSyncError> {
    let payload = serde_json::to_value(payload)
        .map_err(|error| SignedSyncError::Serialization(error.to_string()))?;
    let payload = canonical_json(&payload);
    let mut bytes = Vec::with_capacity(DOMAIN.len() + payload.len() + 64);
    for value in [
        DOMAIN,
        space_id.as_bytes(),
        origin_node_id.as_bytes(),
        recipient_node_id.as_bytes(),
    ] {
        append_bytes(&mut bytes, value);
    }
    bytes.extend_from_slice(&key_epoch.to_be_bytes());
    append_bytes(&mut bytes, message_id.as_bytes());
    append_bytes(&mut bytes, payload.as_bytes());
    Ok(bytes)
}

pub(super) fn decode_fixed<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N], SignedSyncError> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(SignedSyncError::InvalidEncoding { field });
    }
    let mut output = [0; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index * 2;
        *byte =
            (hex_digit(value.as_bytes()[offset]) << 4) | hex_digit(value.as_bytes()[offset + 1]);
    }
    Ok(output)
}

fn append_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("validated lowercase hexadecimal"),
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("JSON string serialization"),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(key, _)| *key);
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("JSON key serialization"),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}
