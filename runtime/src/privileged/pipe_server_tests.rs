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
}

#[test]
fn sddl_rejects_group_and_well_known_sids() {
    // Well-formed SIDs that name groups/shared identities must fall back to
    // SYSTEM-only — granting them would open the pipe to a whole group.
    for group in ["S-1-1-0", "S-1-5-11", "S-1-5-32-544", "S-1-5-18"] {
        assert!(pipe_sddl(group).is_none(), "granted {group:?}");
    }
}

#[test]
fn reclaim_decision_requires_first_instance_flag() {
    // No open instance (no listener, no workers) → must reclaim with
    // FILE_FLAG_FIRST_PIPE_INSTANCE; ≥1 live worker means the name is still
    // ours and a normal instance join is safe.
    assert!(must_reclaim_name(0));
    assert!(!must_reclaim_name(1));
    assert!(!must_reclaim_name(64));
}

#[test]
fn sddl_fallback_is_system_only() {
    // Fail closed: a bad SID degrades to a pipe nobody but SYSTEM can use.
    let fallback = pipe_sddl("garbage").unwrap_or_else(|| "D:(A;;GA;;;SY)".into());
    assert_eq!(fallback, "D:(A;;GA;;;SY)");
}
