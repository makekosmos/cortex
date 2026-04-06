use sha2::{Digest, Sha256};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LAN_PORT: u16 = 21531;

fn crockford_index(c: u8) -> Option<u64> {
    CROCKFORD.iter().position(|&b| b == c).map(|i| i as u64)
}

fn is_crockford_char(c: u8) -> bool {
    CROCKFORD.contains(&c)
}

// ---------------------------------------------------------------------------
// Space code generation
// ---------------------------------------------------------------------------

/// Generate a random 12-character Base32-Crockford code.
#[uniffi::export]
pub fn generate_space_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..12)
        .map(|_| CROCKFORD[rng.gen_range(0..32)] as char)
        .collect()
}

// ---------------------------------------------------------------------------
// IPv4 encoding / decoding
// ---------------------------------------------------------------------------

/// Encode an IPv4 address ("192.168.1.70") into 7 Base32-Crockford chars.
pub fn encode_ipv4(ipv4: &str) -> Option<String> {
    let parts: Vec<&str> = ipv4.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let octets: Vec<u32> = parts
        .iter()
        .filter_map(|p| p.parse::<u32>().ok())
        .collect();
    if octets.len() != 4 || octets.iter().any(|&n| n > 255) {
        return None;
    }

    let n = ((octets[0] << 24) | (octets[1] << 16) | (octets[2] << 8) | octets[3]) as u64;
    let shifted = n << 3;

    let mut result = String::with_capacity(7);
    for i in (0..7).rev() {
        let idx = ((shifted >> (i * 5)) & 0x1f) as usize;
        result.push(CROCKFORD[idx] as char);
    }
    Some(result)
}

/// Decode 7 Base32-Crockford chars back to an IPv4 string.
pub fn decode_ipv4(encoded: &str) -> Option<String> {
    let clean: Vec<u8> = encoded.to_uppercase().bytes().collect();
    if clean.len() != 7 {
        return None;
    }
    if !clean.iter().all(|&c| is_crockford_char(c)) {
        return None;
    }

    let mut value: u64 = 0;
    for &c in &clean {
        value = (value << 5) | crockford_index(c)?;
    }
    value >>= 3;

    let parts = [
        (value >> 24) & 0xff,
        (value >> 16) & 0xff,
        (value >> 8) & 0xff,
        value & 0xff,
    ];
    Some(format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], parts[3]))
}

// ---------------------------------------------------------------------------
// Extended code
// ---------------------------------------------------------------------------

/// Build an extended 19-char code = 12-char secret + 7-char encoded IPv4.
pub fn generate_extended_code(code: &str, primary_ipv4: &str) -> Option<String> {
    let encoded = encode_ipv4(primary_ipv4)?;
    let clean: String = code
        .replace(['-', ' '], "")
        .to_uppercase()
        .chars()
        .take(12)
        .collect();
    Some(format!("{clean}{encoded}"))
}

// ---------------------------------------------------------------------------
// Formatting / parsing
// ---------------------------------------------------------------------------

/// Format code with dashes for display.
pub fn format_space_code(code: &str) -> String {
    let clean: String = code.replace(['-', ' '], "").to_uppercase();
    match clean.len() {
        7 => format!("{}-{}", &clean[0..4], &clean[4..7]),
        12 => format!("{}-{}-{}", &clean[0..4], &clean[4..8], &clean[8..12]),
        19 => format!(
            "{}-{}-{}-{}-{}",
            &clean[0..4],
            &clean[4..8],
            &clean[8..12],
            &clean[12..16],
            &clean[16..19]
        ),
        _ => code.to_string(),
    }
}

/// Parse user input -- accepts 7-char and 12-char codes. Returns raw uppercase code or None.
pub fn parse_space_code(input: &str) -> Option<String> {
    let clean: String = input.replace(['-', ' '], "").to_uppercase();
    if clean.len() == 7 || clean.len() == 12 {
        if clean.bytes().all(|c| is_crockford_char(c)) {
            return Some(clean);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Space ID derivation
// ---------------------------------------------------------------------------

/// Derive a stable space ID from a code: SHA-256(uppercase raw) -> first 16 hex chars.
#[uniffi::export]
pub fn derive_space_id(code: &str) -> String {
    let raw: String = code.replace(['-', ' '], "").to_uppercase();
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let result = hasher.finalize();
    let hex: String = result.iter().map(|b| format!("{b:02x}")).collect();
    hex[..16].to_string()
}

// ---------------------------------------------------------------------------
// QR payload
// ---------------------------------------------------------------------------

/// Generate a QR payload URI.
pub fn generate_qr_payload(code: &str, addresses: &[String]) -> String {
    let formatted = format_space_code(code);
    let addrs = addresses.join(",");
    format!("ark://join?code={formatted}&addrs={addrs}")
}

/// Parse a QR payload, extended code, or bare code.
pub fn parse_qr_payload(payload: &str) -> Option<(String, Vec<String>)> {
    // URI format
    if payload.starts_with("ark://join?") {
        let query_string = &payload["ark://join?".len()..];
        let mut code_raw = None;
        let mut addrs_raw = None;

        for part in query_string.split('&') {
            if let Some(val) = part.strip_prefix("code=") {
                code_raw = Some(val.to_string());
            } else if let Some(val) = part.strip_prefix("addrs=") {
                addrs_raw = Some(val.to_string());
            }
        }

        let raw_code = code_raw?;
        let code = parse_space_code(&raw_code)?;
        let addresses: Vec<String> = addrs_raw
            .map(|a| a.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
            .unwrap_or_default();

        return Some((code, addresses));
    }

    // Extended 19-char code
    let clean: String = payload.replace(['-', ' '], "").to_uppercase();
    if clean.len() == 19 && clean.bytes().all(|c| is_crockford_char(c)) {
        let code = clean[..12].to_string();
        let addresses = match decode_ipv4(&clean[12..]) {
            Some(ip) => vec![format!("{ip}:{LAN_PORT}")],
            None => vec![],
        };
        return Some((code, addresses));
    }

    // Bare code
    let code = parse_space_code(payload)?;
    Some((code, vec![]))
}

// ---------------------------------------------------------------------------
// UniFFI-exported wrappers
// ---------------------------------------------------------------------------

/// Normalize user input to a raw uppercase code. Accepts 7-char and 12-char codes.
#[uniffi::export]
pub fn normalize_code(input: &str) -> Option<String> {
    parse_space_code(input)
}

/// Format a raw code with dashes for display.
#[uniffi::export]
pub fn format_code(code: &str) -> String {
    format_space_code(code)
}

/// Check whether the input is a valid space code.
#[uniffi::export]
pub fn is_valid_code(input: &str) -> bool {
    parse_space_code(input).is_some()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_space_code_length() {
        let code = generate_space_code();
        assert_eq!(code.len(), 12);
        assert!(code.bytes().all(|c| is_crockford_char(c)));
    }

    #[test]
    fn test_encode_decode_ipv4_roundtrip() {
        let ip = "192.168.1.70";
        let encoded = encode_ipv4(ip).unwrap();
        assert_eq!(encoded.len(), 7);
        let decoded = decode_ipv4(&encoded).unwrap();
        assert_eq!(decoded, ip);
    }

    #[test]
    fn test_encode_ipv4_zero() {
        let encoded = encode_ipv4("0.0.0.0").unwrap();
        let decoded = decode_ipv4(&encoded).unwrap();
        assert_eq!(decoded, "0.0.0.0");
    }

    #[test]
    fn test_encode_ipv4_max() {
        let encoded = encode_ipv4("255.255.255.255").unwrap();
        let decoded = decode_ipv4(&encoded).unwrap();
        assert_eq!(decoded, "255.255.255.255");
    }

    #[test]
    fn test_encode_ipv4_invalid() {
        assert!(encode_ipv4("300.0.0.0").is_none());
        assert!(encode_ipv4("not_ip").is_none());
        assert!(encode_ipv4("1.2.3").is_none());
    }

    #[test]
    fn test_decode_ipv4_invalid() {
        assert!(decode_ipv4("SHORT").is_none());
        assert!(decode_ipv4("ABCDEFGH").is_none()); // 8 chars
    }

    #[test]
    fn test_format_space_code_7() {
        assert_eq!(format_space_code("ABCDEFG"), "ABCD-EFG");
    }

    #[test]
    fn test_format_space_code_12() {
        assert_eq!(format_space_code("ABCDEFGHJKMN"), "ABCD-EFGH-JKMN");
    }

    #[test]
    fn test_format_space_code_19() {
        assert_eq!(
            format_space_code("ABCDEFGHJKMNPQRSTVW"),
            "ABCD-EFGH-JKMN-PQRS-TVW"
        );
    }

    #[test]
    fn test_parse_space_code_12() {
        assert_eq!(
            parse_space_code("ABCD-EFGH-JKMN"),
            Some("ABCDEFGHJKMN".to_string())
        );
    }

    #[test]
    fn test_parse_space_code_7() {
        assert_eq!(
            parse_space_code("ABCD-EFG"),
            Some("ABCDEFG".to_string())
        );
    }

    #[test]
    fn test_parse_space_code_invalid() {
        // 'I' is not in Crockford alphabet
        assert!(parse_space_code("ABCDEFGHIJKL").is_none());
        // Wrong length
        assert!(parse_space_code("ABCDE").is_none());
    }

    #[test]
    fn test_derive_space_id() {
        let id = derive_space_id("ABCDEFGHJKMN");
        assert_eq!(id.len(), 16);
        // All hex chars
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        // Deterministic
        assert_eq!(id, derive_space_id("ABCD-EFGH-JKMN"));
    }

    #[test]
    fn test_qr_payload_roundtrip() {
        let code = "ABCDEFGHJKMN";
        let addresses = vec![
            "192.168.1.70:21531".to_string(),
            "10.0.0.5:21531".to_string(),
        ];
        let payload = generate_qr_payload(code, &addresses);
        assert!(payload.starts_with("ark://join?"));

        let (parsed_code, parsed_addrs) = parse_qr_payload(&payload).unwrap();
        assert_eq!(parsed_code, "ABCDEFGHJKMN");
        assert_eq!(parsed_addrs.len(), 2);
        assert!(parsed_addrs.contains(&"192.168.1.70:21531".to_string()));
    }

    #[test]
    fn test_parse_qr_payload_extended_code() {
        let code = "ABCDEFGHJKMN";
        let encoded_ip = encode_ipv4("192.168.1.70").unwrap();
        let extended = format!("{code}{encoded_ip}");

        let (parsed_code, addresses) = parse_qr_payload(&extended).unwrap();
        assert_eq!(parsed_code, code);
        assert_eq!(addresses.len(), 1);
        assert!(addresses[0].starts_with("192.168.1.70:"));
    }

    #[test]
    fn test_parse_qr_payload_bare_code() {
        let (code, addrs) = parse_qr_payload("ABCD-EFGH-JKMN").unwrap();
        assert_eq!(code, "ABCDEFGHJKMN");
        assert!(addrs.is_empty());
    }

    #[test]
    fn test_generate_extended_code() {
        let ext = generate_extended_code("ABCDEFGHJKMN", "192.168.1.70").unwrap();
        assert_eq!(ext.len(), 19);

        // Should be parseable as QR payload
        let (code, addrs) = parse_qr_payload(&ext).unwrap();
        assert_eq!(code, "ABCDEFGHJKMN");
        assert_eq!(addrs.len(), 1);
    }
}
