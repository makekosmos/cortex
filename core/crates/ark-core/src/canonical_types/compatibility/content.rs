use super::shared::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
pub fn note(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    o.insert(
        "description".into(),
        string(m, "description", &[], u, true)?,
    );
    put_extensions(o);
    relation_with_aliases(
        m,
        "relatedNotes",
        &["related_notes"],
        "related",
        LinkWrite { links: l,
        id,
        at,
        unknown: u },
    )?;
    relation_with_aliases(
        m,
        "tagIds",
        &["tag_ids"],
        "tag",
        LinkWrite { links: l,
        id,
        at,
        unknown: u },
    )
}
pub fn person(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    for (c, a) in [
        ("firstName", "first_name"),
        ("lastName", "last_name"),
        ("patronymic", "patronymic"),
        ("birthDate", "birth_date"),
    ] {
        o.insert(c.into(), string(m, c, &[a], u, true)?);
    }
    put_extensions(o);
    relation_single_with_aliases(
        m,
        "photoId",
        &["photo_id"],
        "photo",
        LinkWrite { links: l,
        id,
        at,
        unknown: u },
    )
}
pub fn image(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
) -> Result<(), CompatFailure> {
    for (c, a) in [
        ("fileName", "file_name"),
        ("mimeType", "mime_type"),
        ("sizeBytes", "size_bytes"),
        ("width", "width"),
        ("height", "height"),
        ("resolution", "resolution"),
    ] {
        let v = val(m, c, &[a], u)?;
        if let Some(v) = v {
            if c == "sizeBytes" || c == "width" || c == "height" {
                if !v.is_i64() && !v.is_u64() {
                    return Err(invalid(&format!("/props/{c}")));
                }
            } else if !v.is_string() {
                return Err(invalid(&format!("/props/{c}")));
            }
        }
        o.insert(c.into(), v.cloned().unwrap_or(Value::Null));
    }
    o.insert(
        "altText".into(),
        match val(m, "altText", &["alt_text"], u)? {
            Some(v) if v.is_string() => v.clone(),
            Some(_) => return Err(invalid("/props/altText")),
            None => json!(""),
        },
    );
    put_extensions(o);
    if let Some(v) = val(m, "sourcePath", &["source_path"], u)? {
        local
            .entry("image")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .map(|nested| nested.insert("sourcePath".into(), v.clone()));
    }
    if let Some(v) = val(m, "image", &[], u)? {
        if v.as_str()
            .is_some_and(|s| s.starts_with('/') || s.starts_with("file:") || s.starts_with("\\\\"))
        {
            local
                .entry("image")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .map(|nested| nested.insert("sourcePath".into(), v.clone()));
        } else {
            q.insert("image".into(), v.clone());
        }
    }
    Ok(())
}
pub fn book(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    u: &mut BTreeSet<String>,
    l: &mut Vec<crate::types::ObjectLink>,
    q: &mut Map<String, Value>,
    id: &str,
    at: &str,
) -> Result<(), CompatFailure> {
    for (c, a) in [
        ("author", "author"),
        ("isbn", "isbn"),
        ("pageCount", "page_count"),
        ("language", "language"),
        ("publisher", "publisher"),
        ("publishedDate", "published_date"),
        ("sourceUrl", "source_url"),
    ] {
        let v = val(m, c, &[a], u)?;
        if let Some(v) = v {
            if c == "pageCount" && !v.is_i64() && !v.is_u64() {
                return Err(invalid(&format!("/props/{c}")));
            }
            if c != "pageCount" && !v.is_string() && !v.is_null() {
                return Err(invalid(&format!("/props/{c}")));
            }
        }
        o.insert(c.into(), v.cloned().unwrap_or(Value::Null));
    }
    put_extensions(o);
    if let Some(v) = val(m, "coverImage", &["cover_image"], u)? {
        q.insert("coverImage".into(), v.clone());
    }
    relation_with_aliases(
        m,
        "tagIds",
        &["tag_ids"],
        "tag",
        LinkWrite { links: l,
        id,
        at,
        unknown: u },
    )?;
    relation_with_aliases(
        m,
        "authorPersonIds",
        &["author_person_ids"],
        "author-person",
        LinkWrite { links: l,
        id,
        at,
        unknown: u },
    )
}
