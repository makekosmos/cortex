//! Bounded request reads for the named-pipe boundary.
//!
//! Reading is capped — otherwise any process able to open the pipe could make
//! the LocalSystem service buffer an unbounded stream and exhaust memory.

/// Maximum size of a single request.
pub const MAX_REQUEST_BYTES: u64 = 64 * 1024;

/// Reads one request line from `reader`, at most `MAX_REQUEST_BYTES`.
/// An over-limit request is truncated at the boundary and deterministically
/// becomes invalid JSON → `Response::err`; the rest of the stream is not
/// consumed (the caller closes the pipe after responding).
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
        let mut cursor = std::io::Cursor::new(b"{\"op\":\"ping\"}\n{\"op\":\"reset\"}\n".to_vec());
        let raw = read_request_line(&mut cursor).unwrap();
        assert_eq!(raw.trim(), r#"{"op":"ping"}"#);
    }

    #[test]
    fn read_request_line_caps_unbounded_stream() {
        // Regression: buffering the whole request line lets a peer sending a
        // never-newlined stream grow service memory without limit.
        let flood = vec![b'a'; MAX_REQUEST_BYTES as usize * 4];
        let mut cursor = std::io::Cursor::new(flood);
        let raw = read_request_line(&mut cursor).unwrap();
        assert_eq!(raw.len(), MAX_REQUEST_BYTES as usize);
    }
}
