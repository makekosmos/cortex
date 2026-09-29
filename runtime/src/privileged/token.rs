//! Windows token helpers for the privileged install path.

#![cfg(windows)]

use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// The enabling user's SID — the elevated install runs *as* that user, so the
/// current process token carries it. This SID lands in the service's launch
/// arguments and gates the pipe DACL.
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
            let mut len = 0usize;
            while *sid_w.0.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(sid_w.0, len));
            let _ = LocalFree(HLOCAL(sid_w.0.cast()));
            Ok(text)
        })();
        let _ = CloseHandle(token);
        result
    }
}
