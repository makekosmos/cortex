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
    let octets: Vec<u32> = parts.iter().filter_map(|p| p.parse::<u32>().ok()).collect();
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
    Some(format!(
        "{}.{}.{}.{}",
        parts[0], parts[1], parts[2], parts[3]
    ))
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
    // The match arms key off byte length and slice at fixed offsets — a
    // multi-byte char straddling a cut point (e.g. "ABCÉDE" is 7 bytes) would
    // panic. Non-ASCII input can't be a space code anyway; return it as-is.
    if !clean.is_ascii() {
        return code.to_string();
    }
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
    if (clean.len() == 7 || clean.len() == 12) && clean.bytes().all(is_crockford_char) {
        return Some(clean);
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
    if let Some(query_string) = payload.strip_prefix("ark://join?") {
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
            .map(|a| {
                a.split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        return Some((code, addresses));
    }

    // Extended 19-char code
    let clean: String = payload.replace(['-', ' '], "").to_uppercase();
    if clean.len() == 19 && clean.bytes().all(is_crockford_char) {
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
    include!("space/tests.rs");
}
