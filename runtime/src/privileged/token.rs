//! Windows token + SID helpers for the privileged install path.

#![cfg(windows)]

use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSidToSidW};
use windows::Win32::Security::{GetTokenInformation, TokenUser, PSID, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The current process's user SID — used by the *unelevated* Engine
/// (`client::enable`) to name the account that may use the pipe.
pub fn current_user_sid() -> Result<String, String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| format!("open process token failed: {e}"))?;
        let result = (|| {
            let mut needed: u32 = 0;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
            if needed == 0 || needed > 4096 {
                return Err("token user size unavailable".to_string());
            }
            let mut buf = vec![0u8; needed as usize];
            GetTokenInformation(
                token,
                TokenUser,
                Some(buf.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
            .map_err(|e| format!("token user query failed: {e}"))?;
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut sid_w = windows::core::PWSTR::null();
            ConvertSidToStringSidW(user.User.Sid, &mut sid_w)
                .map_err(|e| format!("sid to string failed: {e}"))?;
            let text = pwstr_to_string(sid_w);
            let _ = LocalFree(HLOCAL(sid_w.0.cast()));
            Ok(text)
        })();
        let _ = CloseHandle(token);
        result
    }
}

fn pwstr_to_string(p: windows::core::PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *p.0.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p.0, len))
    }
}

/// Well-formed SID literal: `S-<digits>(-<digits>)*`.
pub(crate) fn is_sid_literal(sid: &str) -> bool {
    let rest = match sid.strip_prefix("S-") {
        Some(r) => r,
        None => return false,
    };
    !rest.is_empty()
        && rest.len() <= 64
        && rest
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// `true` iff `sid` names an *individual user account* — the only identity
/// allowed on the pipe DACL. Anything else would grant a group or a shared
/// identity access to the SYSTEM service (`--grant-sid S-1-1-0` would open
/// the pipe to Everyone).
///
/// Accepted shapes:
/// * `S-1-5-21-<a>-<b>-<c>-<rid>` — local/domain user accounts;
/// * `S-1-12-1-<…>` — Entra ID accounts.
///
/// Rejected: every well-known/group/builtin SID (`S-1-1-0`, `S-1-5-11`,
/// `S-1-5-32-*`, `S-1-5-18/19/20`, `S-1-5-4`, `S-1-2-0`, …) and machine SIDs
/// (`S-1-5-21-a-b-c` — no RID component).
pub(crate) fn is_user_account_sid(sid: &str) -> bool {
    let parts: Vec<&str> = sid.split('-').collect();
    let digits = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    match parts.as_slice() {
        // S-1-5-21-<a>-<b>-<c>-<rid> — exactly 4 numeric parts after "21".
        ["S", "1", "5", "21", a, b, c, rid] => [a, b, c, rid].iter().all(|p| digits(*p)),
        // S-1-12-1-<subauthorities…> — Entra ID, at least one numeric part.
        ["S", "1", "12", "1", rest @ ..] => !rest.is_empty() && rest.iter().all(digits),
        _ => false,
    }
}

/// Validate a `--grant-sid` argument: literal shape AND an individual
/// user-account SID AND a real SID per the
/// `ConvertStringSidToSidW`/`ConvertSidToStringSidW` round-trip — the string
/// lands in a service launch argument and gates the pipe DACL, so anything
/// malformed, non-canonical, or naming a non-user principal is refused
/// outright.
pub fn validate_grant_sid(sid: &str) -> bool {
    if !is_sid_literal(sid) || !is_user_account_sid(sid) {
        return false;
    }
    unsafe {
        let wide: Vec<u16> = sid.encode_utf16().chain(std::iter::once(0)).collect();
        let mut psid = PSID::default();
        if ConvertStringSidToSidW(windows::core::PCWSTR(wide.as_ptr()), &mut psid).is_err() {
            return false;
        }
        let round_trips = (|| {
            let mut back = windows::core::PWSTR::null();
            if ConvertSidToStringSidW(psid, &mut back).is_err() {
                return false;
            }
            let text = pwstr_to_string(back);
            let _ = LocalFree(HLOCAL(back.0.cast()));
            text == sid
        })();
        let _ = LocalFree(HLOCAL(psid.0));
        round_trips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_literal_shape_is_checked() {
        for bad in [
            "",
            "S-1-5-21-1;)(A;;GA;;;WD",
            "S-1-5-21-abc",
            "Administrators",
            "S--1",
            "S-",
            "S-1-x",
            "not-a-sid",
        ] {
            assert!(!is_sid_literal(bad), "accepted {bad:?}");
            assert!(!validate_grant_sid(bad), "validated {bad:?}");
        }
        for good in [
            "S-1-5-21-3623811015-3361044348-30300820-1013",
            "S-1-12-1-1234567890-1234567890-1234567890-1234",
        ] {
            assert!(is_sid_literal(good));
            assert!(validate_grant_sid(good));
        }
    }

    #[test]
    fn well_known_and_group_sids_are_rejected() {
        // Syntactically valid SIDs that are NOT individual user accounts —
        // granting any of these would open the SYSTEM pipe to a group or a
        // shared identity.
        for bad in [
            "S-1-1-0",              // Everyone
            "S-1-2-0",              // Local
            "S-1-5-4",              // Interactive
            "S-1-5-11",             // Authenticated Users
            "S-1-5-18",             // LocalSystem
            "S-1-5-19",             // LocalService
            "S-1-5-20",             // NetworkService
            "S-1-5-32-544",         // Administrators
            "S-1-5-32-545",         // Users
            "S-1-5-32-547",         // Power Users
            "S-1-5-21-1-2-3",       // machine SID — no RID component
            "S-1-5-21-1-2-3-500-4", // extra component — not a user SID
            "S-1-5-21",             // bare domain prefix
            "S-1-12-1",             // bare Entra prefix
            "S-1-12-2-123",         // Entra non-user class
        ] {
            assert!(is_sid_literal(bad), "{bad:?} is not even a SID literal");
            assert!(!validate_grant_sid(bad), "validated {bad:?}");
        }
    }

    #[test]
    fn non_canonical_sids_are_rejected() {
        // Round-trip catches structurally-invalid and non-canonical forms.
        for bad in [
            "S-1-5-21-1-2-3-1001 ",
            "s-1-5-21-1-2-3-1001",
            "S-1-5-21-1-2-3-01001",
            "S-1-5-021-1-2-3-1001",
        ] {
            assert!(!validate_grant_sid(bad), "validated {bad:?}");
        }
    }
}
