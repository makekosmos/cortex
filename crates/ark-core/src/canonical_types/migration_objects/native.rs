use crate::canonical_types::preflight::{SourceKind, SourceRecord};

/// Native inventory rows already carry a lossless envelope. Only their source
/// kind is adapted to the accepted compatibility vocabulary.
pub(crate) fn adapter(record: &SourceRecord) -> (String, Vec<u8>) {
    let kind = match &record.source_kind {
        SourceKind::Native(k) => match k.as_str() {
            "todos" => "task_obj",
            "projects" | "areas" | "headings" => "project_obj",
            "tags" => "tag_obj",
            other => other,
        },
        SourceKind::Legacy(k) => k.as_str(),
    };
    (kind.into(), record.canonical_bytes.clone())
}
