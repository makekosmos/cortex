use super::*;

#[test]
fn sddl_grants_system_and_one_user() {
    let sddl = pipe_sddl("S-1-5-21-3623811015-3361044348-30300820-1013").unwrap();
    assert_eq!(
        sddl,
        "D:(A;;GA;;;SY)(A;;GA;;;S-1-5-21-3623811015-3361044348-30300820-1013)"
    );
}

#[test]
fn sddl_rejects_malformed_sids() {
    // Anything but S-<digits>-... must never reach the security
    // descriptor — a `;` or `)` would corrupt the DACL.
    for bad in [
        "",
        "S-1-5-21-1;)(A;;GA;;;WD",
        "S-1-5-21-abc",
        "Administrators",
        "S--1",
        "S-",
        "S-1-x",
    ] {
        assert!(pipe_sddl(bad).is_none(), "accepted {bad:?}");
    }
    assert!(pipe_sddl("S-1-5-18").is_some());
    assert!(pipe_sddl("S-1-5-32-544").is_some());
}

#[test]
fn sddl_fallback_is_system_only() {
    // Fail closed: a bad SID degrades to a pipe nobody but SYSTEM can use.
    let fallback = pipe_sddl("garbage").unwrap_or_else(|| "D:(A;;GA;;;SY)".into());
    assert_eq!(fallback, "D:(A;;GA;;;SY)");
}
