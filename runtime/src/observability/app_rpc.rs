//! Rejection logging for app-facing RPC entry points (KOS-298).
//!
//! During KOS-295 the Engine log held no record of rejected `upsert_object`
//! calls, so the reason could not be recovered on the user's machine. Every
//! app-RPC rejection now emits one WARN with client class, operation, object
//! type id and the internal reason — metadata only, never payload values.
use std::borrow::Cow;

/// The public app-RPC error classes — the only tokens a client may see
/// (`{ok:false, error}` replies). Exact match on the Engine contract;
/// anything else collapses to `unavailable`. Package workers answer bare
/// class names wrapped in a `package worker: ` prefix.
pub fn app_error_class(error: &str) -> &'static str {
    let code = error.strip_prefix("package worker: ").unwrap_or(error);
    match code {
        "forbidden" => "forbidden",
        "invalid-request" => "invalid-request",
        "not-found" => "not-found",
        "conflict" => "conflict",
        "timeout" => "timeout",
        "unavailable" => "unavailable",
        _ => "unavailable",
    }
}

/// Reason text for the rejection log — this is what makes "no payload
/// values" a structural guarantee rather than a habit.
///
/// `Site` is a call-site constant explaining where the request died; safe
/// by construction. `Dispatch` is a string produced by the dispatch layer —
/// ark-core codes, ark-host messages or package-worker replies — and may
/// quote payload values, so only a pure wire code is echoed; anything else
/// collapses to its public class.
pub enum RejectionReason<'a> {
    /// Call-site tag, e.g. `RejectionReason::Site("dispatch task failed")`.
    Site(&'static str),
    /// Error text produced while dispatching the operation.
    Dispatch(&'a str),
}

impl RejectionReason<'_> {
    fn text(&self) -> Cow<'_, str> {
        match self {
            Self::Site(tag) => Cow::Borrowed(*tag),
            Self::Dispatch(error) => {
                let code = error.strip_prefix("package worker: ").unwrap_or(error);
                if is_wire_code(code) {
                    Cow::Borrowed(*error)
                } else {
                    // A free-form message can embed a rejected value — the
                    // public class is all the log may keep.
                    Cow::Borrowed(app_error_class(error))
                }
            }
        }
    }
}

/// Internal rejection reasons are wire codes — `:`-separated tokens with an
/// optional `/json-pointer` tail, e.g.
/// `canonical_ingress:invalid_request:canonical_field:/recurrence/dayOfMonth`
/// — or a bare public class name. No whitespace, quotes or prose can be part
/// of one; anything else (a serde message, a quoted value) is free-form and
/// may embed a rejected payload value.
fn is_wire_code(code: &str) -> bool {
    let bare_class = matches!(
        code,
        "forbidden" | "invalid-request" | "not-found" | "conflict" | "timeout" | "unavailable"
    );
    (code.contains(':') || bare_class)
        && code
            .split(':')
            .all(|segment| !segment.is_empty() && segment.bytes().all(is_code_byte))
}

fn is_code_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b'~')
}

/// One WARN per rejected app RPC.
///
/// Callers pass metadata only — client class, operation, object type id,
/// the reason via `RejectionReason`. Never payload values: titles, notes
/// and field contents must not reach the log.
pub fn log_app_rpc_rejection(
    client_class: &str,
    operation: &str,
    type_id: Option<&str>,
    reason: RejectionReason<'_>,
) {
    tracing::warn!(
        client_class = %client_class,
        operation = %operation,
        type_id = %type_id.unwrap_or("-"),
        reason = %reason.text(),
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

// pub(crate) so the end-to-end rejection test in engine_api can capture
// log lines emitted inside the spawned server task.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Shared-buffer writer for asserting on emitted log lines. The
    /// workspace's only other tracing test hook is `with_test_writer`
    /// (tests/it), which writes to stdout and cannot be asserted on.
    #[derive(Clone, Default)]
    pub(crate) struct CaptureBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

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
        pub(crate) fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()).unwrap()
        }
    }

    /// Wire-code reasons are logged verbatim; a free-form error string —
    /// the kind that can quote a rejected payload value — collapses to its
    /// public class.
    #[test]
    fn dispatch_reason_keeps_codes_and_drops_prose() {
        assert_eq!(
            RejectionReason::Dispatch(
                "canonical_ingress:invalid_request:canonical_field:/recurrence/dayOfMonth"
            )
            .text(),
            "canonical_ingress:invalid_request:canonical_field:/recurrence/dayOfMonth"
        );
        assert_eq!(
            RejectionReason::Dispatch("package worker: forbidden").text(),
            "package worker: forbidden"
        );
        assert_eq!(
            RejectionReason::Dispatch("object_conflict:stale_snapshot").text(),
            "object_conflict:stale_snapshot"
        );
        // Prose — even prose built from a payload value — never reaches the log.
        assert_eq!(
            RejectionReason::Dispatch("\"SECRET-VALUE-123\" is not a date-time").text(),
            "unavailable"
        );
        assert_eq!(
            RejectionReason::Dispatch("SECRET-VALUE-123").text(),
            "unavailable"
        );
        assert_eq!(
            RejectionReason::Dispatch("failed to open ARK database at C:\\Users\\a\\ark.db").text(),
            "unavailable"
        );
        assert_eq!(
            RejectionReason::Site("dispatch task failed").text(),
            "dispatch task failed"
        );
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
                RejectionReason::Dispatch(
                    "canonical_ingress:invalid_request:canonical_field:/recurrence/dayOfMonth",
                ),
            );
            // A reason that embeds a payload value collapses to the class.
            log_app_rpc_rejection(
                "memoria-gpui",
                "upsert_object",
                app_rpc_type_id(&params),
                RejectionReason::Dispatch("rejected value PRIVATE TITLE at /title"),
            );
            // No type id → the field is still emitted, empty.
            log_app_rpc_rejection(
                "agenda-gpui",
                "list_objects",
                None,
                RejectionReason::Dispatch("forbidden"),
            );
        });
        let line = buf.text();
        assert!(line.contains("app RPC rejected"));
        assert!(line.contains("memoria-gpui"));
        assert!(line.contains("upsert_object"));
        assert!(line.contains("com.kosmos.note"));
        assert!(line.contains("canonical_ingress:invalid_request"));
        assert!(line.contains("reason=unavailable"));
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
    fn app_error_class_matches_the_public_contract() {
        assert_eq!(app_error_class("package worker: forbidden"), "forbidden");
        assert_eq!(app_error_class("timeout"), "timeout");
        assert_eq!(app_error_class("C:\\private\\path"), "unavailable");
        assert_eq!(
            app_error_class("canonical_ingress:invalid_request:canonical_field:/x"),
            "unavailable"
        );
    }
}
