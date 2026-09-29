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

/// Validate a `--grant-sid` argument: literal shape AND a real SID per the
/// `ConvertStringSidToSidW`/`ConvertSidToStringSidW` round-trip — the string
/// lands in a service launch argument and gates the pipe DACL, so anything
/// malformed or non-canonical is refused outright.
pub fn validate_grant_sid(sid: &str) -> bool {
    if !is_sid_literal(sid) {
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
            "S-1-5-18",
            "S-1-5-32-544",
            "S-1-5-21-3623811015-3361044348-30300820-1013",
        ] {
            assert!(is_sid_literal(good));
            assert!(validate_grant_sid(good));
        }
    }

    #[test]
    fn non_canonical_sids_are_rejected() {
        // Round-trip catches structurally-invalid and non-canonical forms.
        for bad in ["S-1-5-18 ", "s-1-5-18", "S-1-5-018", "S-01-5-18"] {
            assert!(!validate_grant_sid(bad), "validated {bad:?}");
        }
    }
}
