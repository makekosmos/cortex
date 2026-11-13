use super::*;

#[test]
fn parses_default_semicolon_hotkey() {
    let spec = parse_hotkey("Ctrl+Shift+;").unwrap();
    assert_eq!(spec.key_code, 41);
    assert!(spec.ctrl);
    assert!(spec.shift);
    assert!(!spec.cmd);
    assert!(!spec.alt);
}

#[test]
fn parses_double_fn_hotkey() {
    let spec = parse_hotkey("FnFn").unwrap();
    assert!(spec.double_fn);
    assert_eq!(spec.key_code, 63); // kVK_Function
}

#[test]
fn parses_command_space_hotkey() {
    let spec = parse_hotkey("Cmd+Space").unwrap();
    assert_eq!(spec.key_code, 49);
    assert!(spec.cmd);
    assert!(!spec.ctrl);
}

#[test]
fn capture_builds_letter_accelerator() {
    // Ctrl+Shift+H — keyCode 4 = "H", cmd→Super отсутствует.
    let acc = capture_value_to_accelerator(&json!({
        "captured": true, "keyCode": 4,
        "cmd": false, "ctrl": true, "alt": false, "shift": true, "fn": false,
    }));
    assert_eq!(acc.as_deref(), Some("Ctrl+Shift+H"));
}

#[test]
fn capture_maps_cmd_to_super_and_symbol() {
    // Cmd+; — keyCode 41 = ";", cmd мапится в Super.
    let acc = capture_value_to_accelerator(&json!({
        "captured": true, "keyCode": 41,
        "cmd": true, "ctrl": false, "alt": false, "shift": false, "fn": false,
    }));
    assert_eq!(acc.as_deref(), Some("Super+;"));
}

#[test]
fn capture_unknown_keycode_is_ignored() {
    let acc = capture_value_to_accelerator(&json!({
        "captured": true, "keyCode": 9999,
        "cmd": true, "ctrl": false, "alt": false, "shift": false, "fn": false,
    }));
    assert!(acc.is_none());
}

#[test]
fn capture_accelerator_roundtrips_through_parse() {
    // Собранная строка должна обратно парситься в тот же spec.
    let acc = capture_value_to_accelerator(&json!({
        "captured": true, "keyCode": 49,
        "cmd": true, "ctrl": false, "alt": false, "shift": false, "fn": false,
    }))
    .unwrap();
    let spec = parse_hotkey(&acc).unwrap();
    assert_eq!(spec.key_code, 49);
    assert!(spec.cmd);
}

/// Every keyCode mac_key_name emits must round-trip through
/// parse_hotkey — a name parse_hotkey can't read would make the captured
/// hotkey silently un-armable.
#[test]
fn every_captured_name_parses_back() {
    for code in [
        0u16, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
        48, 49, 50, 51, 96, 97, 98, 99, 100, 101, 103, 105, 106, 107, 109, 111, 113, 115, 116, 117,
        118, 119, 120, 121, 122, 123, 124, 125, 126,
    ] {
        let acc = capture_value_to_accelerator(&json!({
            "captured": true, "keyCode": code,
            "cmd": true, "ctrl": false, "alt": false, "shift": false, "fn": false,
        }))
        .unwrap_or_else(|| panic!("mac_key_name({code}) returned None"));
        let spec = parse_hotkey(&acc)
            .unwrap_or_else(|| panic!("captured accelerator {acc:?} does not parse"));
        assert_eq!(spec.key_code, code, "accelerator {acc:?}");
    }
}
