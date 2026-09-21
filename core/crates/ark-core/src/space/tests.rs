use super::*;

#[test]
fn test_generate_space_code_length() {
    let code = generate_space_code();
    assert_eq!(code.len(), 12);
    assert!(code.bytes().all(is_crockford_char));
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
    assert!(decode_ipv4("ABCDEFGH").is_none());
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
    assert_eq!(parse_space_code("ABCD-EFG"), Some("ABCDEFG".to_string()));
}

#[test]
fn test_parse_space_code_invalid() {
    assert!(parse_space_code("ABCDEFGHIJKL").is_none());
    assert!(parse_space_code("ABCDE").is_none());
}

#[test]
fn test_derive_space_id() {
    let id = derive_space_id("ABCDEFGHJKMN");
    assert_eq!(id.len(), 16);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
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
    let (code, addrs) = parse_qr_payload(&ext).unwrap();
    assert_eq!(code, "ABCDEFGHJKMN");
    assert_eq!(addrs.len(), 1);
}
