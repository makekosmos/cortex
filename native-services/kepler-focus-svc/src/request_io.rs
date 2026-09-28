//! Bounded request reads for the named-pipe boundary.
//!
//! Pipe открыт всем Authenticated Users, поэтому чтение обязано быть
//! ограничено — иначе любой локальный процесс может заставить LocalSystem
//! service буферизовать неограниченный поток и исчерпать память.

/// Максимальный размер одного request'а.
pub const MAX_REQUEST_BYTES: u64 = 64 * 1024;

/// Читает одну request line из `reader`, не более `MAX_REQUEST_BYTES`.
/// Over-limit request обрезается на границе и детерминированно становится
/// invalid JSON → `Response::err`; остаток стрима не дочитывается (caller
/// закрывает pipe после ответа).
pub fn read_request_line(reader: &mut impl std::io::Read) -> std::io::Result<String> {
    use std::io::{BufRead, Read};
    let mut raw = String::new();
    std::io::BufReader::new(reader)
        .take(MAX_REQUEST_BYTES)
        .read_line(&mut raw)?;
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_request_line_reads_one_complete_line() {
        let mut cursor = std::io::Cursor::new(b"{\"op\":\"ping\"}\n{\"op\":\"status\"}\n".to_vec());
        let raw = read_request_line(&mut cursor).unwrap();
        assert_eq!(raw.trim(), r#"{"op":"ping"}"#);
    }

    #[test]
    fn read_request_line_caps_unbounded_stream() {
        // Regression: handle_connection buffered the whole request line — a
        // peer sending a never-newlined stream grew service memory unbounded.
        let flood = vec![b'a'; MAX_REQUEST_BYTES as usize * 4];
        let mut cursor = std::io::Cursor::new(flood);
        let raw = read_request_line(&mut cursor).unwrap();
        assert_eq!(raw.len(), MAX_REQUEST_BYTES as usize);
    }
}
