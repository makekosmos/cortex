//! The pre-install consent surface for `packages.disclosure` payloads.
//!
//! The payload is the signed permission contract (runtime
//! `PackageDisclosure`), not store copy. It is rendered here as a Russian
//! list rather than raw JSON, and any part that cannot be fully translated
//! fails the whole render so the caller can block install.
use mundus_gpui_kit::fields::vopt;
use serde_json::Value;

/// Why the consent screen refuses to render a disclosure payload.
/// Consent means "the user saw everything the package asked for" — a payload
/// the Manager cannot fully translate is not consent, so install is blocked.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConsentBlock {
    /// The payload shape does not match the disclosure contract.
    Malformed,
    /// A capability/action/direction this Manager version cannot explain.
    UnknownPermission,
}

impl ConsentBlock {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            // New kinds ship with newer manifests; a Manager that cannot
            // name them must not wave them through silently.
            ConsentBlock::UnknownPermission => {
                "Пакет запрашивает неизвестное разрешение — установка невозможна, обновите Mundus."
            }
            ConsentBlock::Malformed => {
                "Не удалось прочитать список разрешений — установка невозможна."
            }
        }
    }
}

fn blocked<T>(block: ConsentBlock) -> Result<T, ConsentBlock> {
    Err(block)
}

/// The capability vocabulary is the closed list the Engine validates
/// (`known_capability` in runtime/src/package_manifest.rs). Keep this match
/// exhaustive over that list: anything else must block consent, not render.
fn capability_text(capability: &str) -> Result<&'static str, ConsentBlock> {
    match capability {
        "ark.read" => Ok("Читать записи из хранилища"),
        "ark.write" => Ok("Создавать и изменять записи в хранилище"),
        "launcher.search" => Ok("Показывать свои результаты в поиске"),
        "network" => Ok("Выходить в интернет"),
        "filesystem.read" => Ok("Читать файлы"),
        "filesystem.write" => Ok("Изменять файлы"),
        "clipboard" => Ok("Работать с буфером обмена"),
        "notifications" => Ok("Показывать уведомления"),
        "process.spawn" => Ok("Запускать программы"),
        "worker.invoke" => Ok("Вызывать операции других компонентов"),
        "dictation.control" => Ok("Управлять диктовкой"),
        _ => blocked(ConsentBlock::UnknownPermission),
    }
}

/// DataAction serializes lowercase (`package_manifest.rs`).
fn data_action_text(action: &str) -> Result<&'static str, ConsentBlock> {
    match action {
        "read" => Ok("читать"),
        "create" => Ok("создавать"),
        "update" => Ok("изменять"),
        "delete" => Ok("удалять"),
        "subscribe" => Ok("следить за изменениями"),
        "link" => Ok("связывать"),
        _ => blocked(ConsentBlock::UnknownPermission),
    }
}

/// MappingDirection serializes camelCase, MappingFidelity kebab-case.
fn mapping_direction_text(direction: &str) -> Result<&'static str, ConsentBlock> {
    match direction {
        "import" => Ok("импортирует в Mundus"),
        "export" => Ok("экспортирует из Mundus"),
        "bidirectionalSync" => Ok("синхронизирует в обе стороны"),
        _ => blocked(ConsentBlock::UnknownPermission),
    }
}

fn mapping_fidelity_text(fidelity: &str) -> Result<&'static str, ConsentBlock> {
    match fidelity {
        "native" => Ok("в исходном виде"),
        "lossless" => Ok("без потерь"),
        "lossy" => Ok("с потерей части данных"),
        "metadata-only" => Ok("только свойства, без содержимого"),
        _ => blocked(ConsentBlock::UnknownPermission),
    }
}

/// PackageKind serializes lowercase.
fn package_kind_text(kind: &str) -> Result<&'static str, ConsentBlock> {
    match kind {
        "app" => Ok("Приложение"),
        "source" => Ok("Источник данных"),
        "bridge" => Ok("Мост"),
        _ => blocked(ConsentBlock::UnknownPermission),
    }
}

/// Strict accessors: `varr`/`vstr` coerce missing or wrong-typed values into
/// empty results, which would silently drop part of a consent contract.
fn strict_str<'a>(v: &'a Value, key: &str) -> Result<&'a str, ConsentBlock> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(ConsentBlock::Malformed)
}

fn strict_arr<'a>(v: &'a Value, key: &str) -> Result<&'a [Value], ConsentBlock> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(items) => items
            .as_array()
            .map(Vec::as_slice)
            .ok_or(ConsentBlock::Malformed),
    }
}

fn string_list(v: &Value, key: &str) -> Result<Vec<String>, ConsentBlock> {
    strict_arr(v, key)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or(ConsentBlock::Malformed)
        })
        .collect()
}

fn join_scopes(scopes: &[String]) -> String {
    if scopes.is_empty() {
        String::new()
    } else {
        format!(" ({})", scopes.join(", "))
    }
}

/// The signed disclosure contract rendered as a Russian consent list.
/// Every entry in the payload must reach the screen — anything unexpected
/// (unknown kind, malformed field) fails the whole render so install can be
/// blocked instead of silently dropping a permission.
pub(crate) fn consent_body(d: &Value) -> Result<String, ConsentBlock> {
    if !d.is_object() {
        return blocked(ConsentBlock::Malformed);
    }
    let name = vopt(d, "name")
        .filter(|n| !n.is_empty())
        .ok_or(ConsentBlock::Malformed)?;
    let kind = package_kind_text(strict_str(d, "kind")?)?;
    let mut out = format!("«{name}» — {kind}");
    let version = vopt(d, "version").filter(|v| !v.is_empty());
    if let Some(version) = version {
        out.push_str(&format!(", версия {version}"));
    }
    if let Some(publisher) = vopt(d, "publisher").filter(|p| !p.is_empty()) {
        out.push_str(&format!(". Издатель: {publisher}"));
    }
    out.push_str(".\n");

    let mut wrote_header = false;
    let mut section = |out: &mut String| {
        if !wrote_header {
            out.push_str("\nПакет сможет:\n");
            wrote_header = true;
        }
    };

    for cap in strict_arr(d, "capabilities")? {
        let capability = cap
            .get("capability")
            .and_then(Value::as_str)
            .ok_or(ConsentBlock::Malformed)?;
        let text = capability_text(capability)?;
        let scopes = string_list(cap, "scopes")?;
        section(&mut out);
        out.push_str(&format!("• {text}{}\n", join_scopes(&scopes)));
    }
    for rule in strict_arr(d, "data")? {
        let type_id = strict_str(rule, "type")?;
        let mut actions = String::new();
        for (i, action) in strict_arr(rule, "actions")?.iter().enumerate() {
            let action = action.as_str().ok_or(ConsentBlock::Malformed)?;
            if i > 0 {
                actions.push_str(", ");
            }
            actions.push_str(data_action_text(action)?);
        }
        if actions.is_empty() {
            return blocked(ConsentBlock::Malformed);
        }
        section(&mut out);
        out.push_str(&format!("• Данные «{type_id}»: {actions}"));
        let versions = vopt(rule, "versions").filter(|v| !v.is_empty() && *v != "*");
        if let Some(versions) = versions {
            out.push_str(&format!(" (версии {versions})"));
        }
        let fields_read = string_list(rule, "fields_read")?;
        let fields_write = string_list(rule, "fields_write")?;
        if !fields_read.is_empty() {
            out.push_str(&format!("; поля для чтения: {}", fields_read.join(", ")));
        }
        if !fields_write.is_empty() {
            out.push_str(&format!("; поля для записи: {}", fields_write.join(", ")));
        }
        let relations_read = string_list(rule, "relations_read")?;
        let relations_write = string_list(rule, "relations_write")?;
        if !relations_read.is_empty() {
            out.push_str(&format!(
                "; связи для чтения: {}",
                relations_read.join(", ")
            ));
        }
        if !relations_write.is_empty() {
            out.push_str(&format!(
                "; связи для записи: {}",
                relations_write.join(", ")
            ));
        }
        out.push('\n');
    }
    for mapping in strict_arr(d, "mappings")? {
        let type_id = strict_str(mapping, "type")?;
        let direction = mapping_direction_text(strict_str(mapping, "direction")?)?;
        let fidelity = mapping_fidelity_text(strict_str(mapping, "fidelity")?)?;
        section(&mut out);
        out.push_str(&format!("• {direction} данные «{type_id}» {fidelity}\n"));
    }
    if !wrote_header {
        out.push_str("\nПакет не запрашивает дополнительных разрешений.\n");
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    // No `use super::*` here: the parent glob-imports `gpui::*`, which
    // carries a `test` attribute macro that shadows the built-in `#[test]`
    // and blows the recursion limit.
    use super::{consent_body, ConsentBlock};
    use serde_json::json;

    fn disclosure(extra: serde_json::Value) -> serde_json::Value {
        let mut base = json!({
            "id": "com.kosmos.demo",
            "name": "Демо",
            "version": "1.0.0",
            "kind": "source",
            "publisher": "kosmos",
            "capabilities": [{"capability": "network", "scopes": ["api.example.com"]}],
            "data": [{
                "type": "task",
                "versions": ">=1.0.0",
                "actions": ["read", "create"],
                "fields_read": ["props.title"],
                "fields_write": ["props.done"],
                "relations_read": ["parent"],
                "relations_write": []
            }],
            "mappings": [{
                "type": "task",
                "direction": "bidirectionalSync",
                "fidelity": "metadata-only"
            }]
        });
        if let (Some(dst), Some(src)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        base
    }

    #[test]
    fn every_known_capability_has_russian_text() {
        // Mirrors `known_capability` in runtime/src/package_manifest.rs —
        // the closed set the Engine validates.
        let known = [
            "ark.read",
            "ark.write",
            "launcher.search",
            "network",
            "filesystem.read",
            "filesystem.write",
            "clipboard",
            "notifications",
            "process.spawn",
            "worker.invoke",
            "dictation.control",
        ];
        for capability in known {
            let text = consent_body(&disclosure(
                json!({"capabilities": [{"capability": capability}]}),
            ));
            assert!(text.is_ok(), "capability {capability} blocked consent");
        }
    }

    #[test]
    fn unknown_capability_blocks_consent() {
        let result = consent_body(&disclosure(
            json!({"capabilities": [{"capability": "teleport"}]}),
        ));
        assert_eq!(result, Err(ConsentBlock::UnknownPermission));
    }

    #[test]
    fn unknown_enum_values_block_consent() {
        for extra in [
            json!({"kind": "plugin"}),
            json!({"data": [{"type": "task", "versions": "*", "actions": ["annihilate"]}]}),
            json!({"mappings": [{"type": "task", "direction": "sideways", "fidelity": "native"}]}),
            json!({"mappings": [{"type": "task", "direction": "export", "fidelity": "vague"}]}),
        ] {
            assert_eq!(
                consent_body(&disclosure(extra)),
                Err(ConsentBlock::UnknownPermission)
            );
        }
    }

    #[test]
    fn malformed_payload_blocks_consent() {
        assert_eq!(
            consent_body(&json!({"name": "Демо", "kind": "source"})),
            Ok(
                "«Демо» — Источник данных.\n\nПакет не запрашивает дополнительных разрешений."
                    .to_string()
            )
        );
        assert_eq!(
            consent_body(&json!({"kind": "source"})),
            Err(ConsentBlock::Malformed)
        );
        assert_eq!(
            consent_body(&disclosure(json!({"capabilities": "network"}))),
            Err(ConsentBlock::Malformed)
        );
        assert_eq!(
            consent_body(&disclosure(
                json!({"data": [{"type": "task", "versions": "*", "actions": []}]})
            )),
            Err(ConsentBlock::Malformed)
        );
        assert_eq!(consent_body(&json!(null)), Err(ConsentBlock::Malformed));
    }

    #[test]
    fn rendered_consent_has_no_json_or_internal_ids() {
        let text = consent_body(&disclosure(json!({}))).expect("consent text");
        assert!(!text.contains('{') && !text.contains('}'), "{text}");
        assert!(!text.contains("com.kosmos."), "{text}");
        // Enum spellings must never leak through untranslated.
        for raw in [
            "bidirectionalSync",
            "metadata-only",
            "ark.read",
            "read",
            "export",
            "lossy",
        ] {
            assert!(!text.contains(raw), "{raw} leaked into: {text}");
        }
        // Every declared item is present in human form.
        for expected in [
            "api.example.com",
            "task",
            "props.title",
            "props.done",
            "parent",
        ] {
            assert!(text.contains(expected), "missing {expected} in: {text}");
        }
        assert!(text.contains("синхронизирует в обе стороны"), "{text}");
        assert!(text.contains("читать"), "{text}");
    }
}
