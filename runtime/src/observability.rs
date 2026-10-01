use std::collections::VecDeque;
use std::io::Write;
use std::sync::OnceLock;

use regex::Regex;

pub const CORRELATION_ID_ENV: &str = "MUNDUS_CORRELATION_ID";
const MAX_REDACTED_TEXT_BYTES: usize = 16 * 1024;

pub fn correlation_id() -> String {
    std::env::var(CORRELATION_ID_ENV)
        .ok()
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub fn crash_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// One WARN per rejected app RPC — during KOS-295 the Engine log held no
/// record of rejected `upsert_object` calls, so the reason could not be
/// recovered on the user's machine (KOS-298).
///
/// Callers pass metadata only — client class, operation, object type id,
/// the internal reason code (e.g. `canonical_ingress:invalid_request:
/// canonical_field:/recurrence/dayOfMonth`). Never payload values: titles,
/// notes and field contents must not reach the log.
pub fn log_app_rpc_rejection(
    client_class: &str,
    operation: &str,
    type_id: Option<&str>,
    reason: &str,
) {
    tracing::warn!(
        client_class = %client_class,
        operation = %operation,
        type_id = %type_id.unwrap_or("-"),
        reason = %reason,
        "app RPC rejected",
    );
}

/// The object type an app RPC targets: `upsert_object` carries it under
/// `object.typeId`, list/scoped ops at the top level as `type_id`/`typeId`.
/// This is the ONLY params field the rejection log may read.
pub fn app_rpc_type_id(params: &serde_json::Value) -> Option<&str> {
    params
        .get("type_id")
        .or_else(|| params.get("typeId"))
        .or_else(|| {
            params
                .get("object")
                .and_then(|object| object.get("typeId").or_else(|| object.get("type_id")))
        })
        .and_then(serde_json::Value::as_str)
}

pub fn stderr(message: impl AsRef<str>) {
    eprintln!("{}", redact_text(message.as_ref()));
}

pub fn redact_text(input: &str) -> String {
    let mut output = input.to_owned();
    for regex in redaction_patterns() {
        output = regex
            .replace_all(&output, |captures: &regex::Captures<'_>| {
                if captures
                    .get(0)
                    .is_some_and(|value| value.as_str().to_ascii_lowercase().starts_with("bearer "))
                {
                    "Bearer [REDACTED]".to_string()
                } else {
                    "[REDACTED]".to_string()
                }
            })
            .into_owned();
    }
    truncate_utf8(output, MAX_REDACTED_TEXT_BYTES)
}

pub fn redact_log_line(input: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(input) {
        Ok(mut value) => {
            redact_json_value(&mut value, "", 0);
            serde_json::to_string(&value).unwrap_or_else(|_| redact_text(input))
        }
        Err(_) => redact_text(input),
    }
}

fn redact_json_value(value: &mut serde_json::Value, key: &str, depth: usize) {
    if sensitive_key(key) {
        *value = serde_json::Value::String("[REDACTED]".into());
        return;
    }
    if depth >= 6 {
        *value = serde_json::Value::String("[TRUNCATED]".into());
        return;
    }
    match value {
        serde_json::Value::String(text) => *text = redact_text(text),
        serde_json::Value::Array(items) => {
            items.truncate(50);
            for item in items {
                redact_json_value(item, "", depth + 1);
            }
        }
        serde_json::Value::Object(map) => {
            if map.len() > 50 {
                let remove: Vec<String> = map.keys().skip(50).cloned().collect();
                for key in remove {
                    map.remove(&key);
                }
                map.insert(
                    "_truncated".into(),
                    serde_json::Value::String("[TRUNCATED]".into()),
                );
            }
            for (child_key, child_value) in map.iter_mut() {
                redact_json_value(child_value, child_key, depth + 1);
            }
        }
        _ => {}
    }
}

fn sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase()
            .chars()
            .filter(|character| character.is_ascii_alphabetic())
            .collect::<String>()
            .as_str(),
        "apikey"
            | "authorization"
            | "authsecret"
            | "authtoken"
            | "body"
            | "content"
            | "cookie"
            | "data"
            | "dbpath"
            | "password"
            | "payload"
            | "request"
            | "response"
            | "secret"
            | "text"
            | "token"
    )
}

pub struct RedactingWriter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
}

impl<W: Write> RedactingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }

    fn flush_redacted(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return self.inner.flush();
        }
        let text = String::from_utf8_lossy(&self.buffer);
        for line in text.lines() {
            self.inner.write_all(redact_log_line(line).as_bytes())?;
            self.inner.write_all(b"\n")?;
        }
        self.buffer.clear();
        self.inner.flush()
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_redacted()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush_redacted();
    }
}

fn redaction_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+",
            r"(?i)\b(?:authorization|auth[_-]?token|api[_-]?key|secret|password)\s*[:=]\s*[^\s,;]+",
            r"\b[0-9a-fA-F]{64}\b",
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
            r#"(?i)[A-Z]:\\Users\\[^\s"'\\]+(?:\\[^\s"']+)*"#,
            r#"(?:/Users|/home)/[^\s"']+"#,
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("redaction regex is static and valid"))
        .collect()
    })
}

fn truncate_utf8(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push_str("[TRUNCATED]");
    value
}

#[derive(Debug)]
pub struct BoundedTextTail {
    lines: VecDeque<String>,
    bytes: usize,
    max_lines: usize,
    max_bytes: usize,
}

impl BoundedTextTail {
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            max_lines,
            max_bytes,
        }
    }

    pub fn push(&mut self, line: &str) {
        // Structured worker output must use the same key-aware redaction path
        // as persisted logs; plain text still falls back to redact_text.
        let line = redact_log_line(line);
        self.bytes = self.bytes.saturating_add(line.len());
        self.lines.push_back(line);
        while self.lines.len() > self.max_lines || self.bytes > self.max_bytes {
            let Some(removed) = self.lines.pop_front() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(removed.len());
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_supported_sensitive_patterns() {
        let token = "a".repeat(64);
        let input = format!(
            "Bearer abc.def auth_token={token} alice@example.com \
             C:\\Users\\alice\\vault\\note.md /home/alice/vault/note.md"
        );
        let output = redact_text(&input);
        assert!(!output.contains("abc.def"));
        assert!(!output.contains(&token));
        assert!(!output.contains("alice@example.com"));
        assert!(!output.contains("note.md"));
    }

    #[test]
    fn redacts_sensitive_json_keys_before_persistence() {
        let output = redact_log_line(r#"{"scope":"rpc","payload":{"title":"private note"}}"#);
        assert!(!output.contains("private note"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn writer_redacts_before_forwarding() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer
                .write_all(b"{\"token\":\"secret-value\",\"safe\":1}\n")
                .unwrap();
        }
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("secret-value"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn bounded_tail_redacts_and_evicts_old_lines() {
        let mut tail = BoundedTextTail::new(2, 64);
        tail.push("first");
        tail.push("second alice@example.com");
        tail.push("third");
        let snapshot = tail.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert!(!snapshot.join("\n").contains("alice@example.com"));
        assert_eq!(snapshot.last().map(String::as_str), Some("third"));
    }

    /// Shared-buffer writer for asserting on emitted log lines. The
    /// workspace's only other tracing test hook is `with_test_writer`
    /// (tests/it), which writes to stdout and cannot be asserted on.
    #[derive(Clone, Default)]
    struct CaptureBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureBuf {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureBuf {
        type Writer = CaptureBuf;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    impl CaptureBuf {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap()
        }
    }

    /// A rejected app RPC must log client class, op, type id and the
    /// internal reason — and nothing from the payload (KOS-298).
    #[test]
    fn rejection_log_carries_metadata_but_never_payload_values() {
        let buf = CaptureBuf::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(buf.clone())
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            let params = serde_json::json!({
                "object": {
                    "typeId": "com.kosmos.note",
                    "title": "PRIVATE TITLE",
                    "contentJson": {"text": "private body"},
                }
            });
            log_app_rpc_rejection(
                "memoria-gpui",
                "upsert_object",
                app_rpc_type_id(&params),
                "canonical_ingress:invalid_request:canonical_field:/recurrence/dayOfMonth",
            );
            // No type id → the field is still emitted, empty.
            log_app_rpc_rejection("agenda-gpui", "list_objects", None, "forbidden");
        });
        let line = buf.text();
        assert!(line.contains("app RPC rejected"));
        assert!(line.contains("memoria-gpui"));
        assert!(line.contains("upsert_object"));
        assert!(line.contains("com.kosmos.note"));
        assert!(line.contains("canonical_ingress:invalid_request"));
        assert!(line.contains("forbidden"));
        assert!(!line.contains("PRIVATE TITLE"));
        assert!(!line.contains("private body"));
    }

    #[test]
    fn app_rpc_type_id_reads_nested_and_top_level_keys() {
        use serde_json::json;
        assert_eq!(
            app_rpc_type_id(&json!({"object": {"typeId": "a"}})),
            Some("a")
        );
        assert_eq!(app_rpc_type_id(&json!({"type_id": "b"})), Some("b"));
        assert_eq!(app_rpc_type_id(&json!({"typeId": "c"})), Some("c"));
        assert_eq!(app_rpc_type_id(&json!({"id": "n1"})), None);
    }

    #[test]
    fn invalid_env_correlation_is_replaced() {
        let previous = std::env::var(CORRELATION_ID_ENV).ok();
        std::env::set_var(CORRELATION_ID_ENV, "not-a-uuid");
        let value = correlation_id();
        if let Some(previous) = previous {
            std::env::set_var(CORRELATION_ID_ENV, previous);
        } else {
            std::env::remove_var(CORRELATION_ID_ENV);
        }
        assert!(uuid::Uuid::parse_str(&value).is_ok());
    }
}
