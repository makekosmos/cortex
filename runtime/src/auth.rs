// Auth для WS-сессии: bearer token + PID-binding.
//
// AC5 (spec): client передаёт в hello-handshake свой PID, Mundus проверяет что
// процесс существует и принадлежит тому же user. Полностью реализовано для Windows;
// на Unix используем kill(pid, 0) — EPERM сигнализирует foreign-user.

use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("PID {pid} does not exist")]
    PidNotFound { pid: u32 },
    #[error("PID {pid} belongs to a different user")]
    ForeignUserPid { pid: u32 },
    #[error("auth error: {0}")]
    Other(String),
}

/// Генерация 256-битного random token, hex-encoded (64 ASCII chars).
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

/// Constant-time string equality для token comparison.
/// Защищает от timing-side-channel attacks при перебое token.
pub fn validate_token(actual: &str, expected: &str) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let a = actual.as_bytes();
    let b = expected.as_bytes();
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Проверить, что процесс с указанным PID существует и принадлежит current user.
///
/// Возвращает:
///   Ok(())                — процесс жив и same-user.
///   Err(PidNotFound)      — процесса с таким PID нет.
///   Err(ForeignUserPid)   — процесс существует, но принадлежит другому user'у.
#[cfg(windows)]
pub fn validate_pid_belongs_to_current_user(pid: u32) -> Result<(), AuthError> {
    use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER};
    use windows::Win32::Security::EqualSid;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        // Открываем target process. PROCESS_QUERY_LIMITED_INFORMATION — минимальный flag
        // нужный для OpenProcessToken (для protected/system processes доступа всё равно
        // не будет, что в нашем сценарии маркируется как "не наш user").
        let process_handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) if !h.is_invalid() => h,
            Ok(_) | Err(_) => {
                let err = windows::Win32::Foundation::GetLastError();
                if err == ERROR_INVALID_PARAMETER {
                    return Err(AuthError::PidNotFound { pid });
                }
                // Любая другая ошибка (например access denied) интерпретируется как foreign user.
                return Err(AuthError::PidNotFound { pid });
            }
        };

        let result = (|| -> Result<(), AuthError> {
            // Получить SID target процесса.
            let target_sid_buf = get_process_user_sid(process_handle)?;

            // Получить SID текущего процесса.
            let current_process = GetCurrentProcess();
            let current_sid_buf = get_process_user_sid(current_process)?;

            // Сравнить.
            let target_sid_ptr = target_sid_buf.as_ptr() as *const _;
            let current_sid_ptr = current_sid_buf.as_ptr() as *const _;
            let equal = EqualSid(
                windows::Win32::Security::PSID(target_sid_ptr as *mut _),
                windows::Win32::Security::PSID(current_sid_ptr as *mut _),
            );

            if equal.is_ok() {
                Ok(())
            } else {
                Err(AuthError::ForeignUserPid { pid })
            }
        })();

        let _ = CloseHandle(process_handle);
        result
    }
}

#[cfg(windows)]
unsafe fn get_process_user_sid(
    process: windows::Win32::Foundation::HANDLE,
) -> Result<Vec<u8>, AuthError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::OpenProcessToken;

    let mut token_handle = windows::Win32::Foundation::HANDLE::default();
    OpenProcessToken(process, TOKEN_QUERY, &mut token_handle)
        .map_err(|e| AuthError::Other(format!("OpenProcessToken failed: {e:?}")))?;

    let mut size_needed: u32 = 0;
    // Первый вызов — узнать нужный размер буфера.
    let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut size_needed);

    let mut buffer = vec![0u8; size_needed as usize];
    let res = GetTokenInformation(
        token_handle,
        TokenUser,
        Some(buffer.as_mut_ptr() as *mut _),
        size_needed,
        &mut size_needed,
    );

    let _ = CloseHandle(token_handle);

    res.map_err(|e| AuthError::Other(format!("GetTokenInformation failed: {e:?}")))?;

    // TOKEN_USER layout: pointer на SID + attributes. Сам SID лежит сразу после структуры
    // в той же allocation. Извлекаем все байты буфера — EqualSid делает свою проверку shape.
    let token_user_ptr = buffer.as_ptr() as *const TOKEN_USER;
    let sid_ptr = (*token_user_ptr).User.Sid.0 as *const u8;

    // Вычисляем длину SID. SID имеет fixed-size header + variable subauthorities.
    // GetLengthSid даёт точную длину.
    let sid_len =
        windows::Win32::Security::GetLengthSid(windows::Win32::Security::PSID(sid_ptr as *mut _))
            as usize;

    let mut sid_copy = vec![0u8; sid_len];
    std::ptr::copy_nonoverlapping(sid_ptr, sid_copy.as_mut_ptr(), sid_len);
    Ok(sid_copy)
}

/// Executable image path of a same-user process, queried by PID —
/// trustworthy process identity, unlike self-declared client headers.
/// Used to pin `/v1/rpc` callers that claim to be the Manager
/// (KOS-269 round 3).
#[cfg(windows)]
pub fn process_image_path(pid: u32) -> Result<std::path::PathBuf, AuthError> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| AuthError::PidNotFound { pid })?;
        let result = (|| {
            let mut buf = vec![0u16; 1024];
            let mut size = buf.len() as u32;
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .map_err(|e| AuthError::Other(format!("process image query failed: {e}")))?;
            Ok(std::path::PathBuf::from(String::from_utf16_lossy(
                &buf[..size as usize],
            )))
        })();
        let _ = CloseHandle(process);
        result
    }
}

#[cfg(unix)]
pub fn validate_pid_belongs_to_current_user(pid: u32) -> Result<(), AuthError> {
    // kill(pid, 0) → 0  : процесс существует и мы имеем right послать signal (same user или root)
    //              → -1 : errno = ESRCH (no such process) | EPERM (foreign user) | other
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        return Ok(());
    }
    let errno = std::io::Error::last_os_error().raw_os_error();
    match errno {
        Some(libc::ESRCH) => Err(AuthError::PidNotFound { pid }),
        Some(libc::EPERM) => Err(AuthError::ForeignUserPid { pid }),
        Some(other) => Err(AuthError::Other(format!("kill returned errno {other}"))),
        None => Err(AuthError::Other("kill returned -1, no errno".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_64_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn two_tokens_differ() {
        // Очень малая вероятность коллизии 1/2^256.
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
    }

    #[test]
    fn validate_token_matches() {
        let t = generate_token();
        assert!(validate_token(&t, &t));
    }

    #[test]
    fn validate_token_rejects_different() {
        let a = generate_token();
        let b = generate_token();
        assert!(!validate_token(&a, &b));
    }

    #[test]
    fn validate_token_rejects_different_length() {
        assert!(!validate_token(
            "short",
            "much-longer-token-that-doesnt-match"
        ));
    }

    #[test]
    fn current_process_pid_is_ourselves() {
        let pid = std::process::id();
        assert_eq!(validate_pid_belongs_to_current_user(pid), Ok(()));
    }

    #[test]
    fn impossibly_high_pid_is_not_found() {
        // PIDs > 2^22 крайне маловероятны на любой OS.
        let pid = 0x7FFFFFFFu32;
        let result = validate_pid_belongs_to_current_user(pid);
        assert!(matches!(result, Err(AuthError::PidNotFound { .. })));
    }
}
