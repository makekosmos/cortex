
/// The full write bundle threaded through the kind mappers that need every
/// accumulator: canonical `out`, unknown-field sink, links, local-state and
/// quarantine maps, plus the record identity pair.
pub struct CompatCtx<'a> {
    pub out: &'a mut Map<String, Value>,
    pub consumed: &'a mut std::collections::BTreeSet<String>,
    pub links: &'a mut Vec<crate::types::ObjectLink>,
    pub local: &'a mut Map<String, Value>,
    pub quarantine: &'a mut Map<String, Value>,
    pub id: &'a str,
    pub at: &'a str,
}

/// Write targets shared by the relation helpers: the link accumulator plus
/// the record's identity pair and the unknown-field sink.
pub struct LinkWrite<'a> {
    pub links: &'a mut Vec<crate::types::ObjectLink>,
    pub id: &'a str,
    pub at: &'a str,
    pub unknown: &'a mut std::collections::BTreeSet<String>,
}

pub fn relation_with_aliases(
    m: &Map<String, Value>,
    key: &str,
    aliases: &[&str],
    kind: &str,
    w: LinkWrite<'_>,
) -> Result<(), CompatFailure> {
    let LinkWrite {
        links,
        id,
        at,
        unknown: u,
    } = w;
    let pointer = |fallback: &str| {
        if m.contains_key(key) {
            format!("/props/{key}")
        } else {
            aliases
                .iter()
                .find(|a| m.contains_key(**a))
                .map_or_else(|| format!("/props/{fallback}"), |a| format!("/props/{a}"))
        }
    };
    if let Some(v) = relation_val(m, key, aliases, u)? {
        let mut ids = Vec::new();
        if let Some(s) = v.as_str() {
            ids.push(s.to_owned())
        } else if let Some(a) = v.as_array() {
            for x in a {
                ids.push(x.as_str().ok_or_else(|| invalid(&pointer(key)))?.to_owned())
            }
        } else {
            return Err(invalid(&pointer(key)));
        }
        ids.sort();
        ids.dedup();
        for target in ids {
            let mut h = Sha256::new();
            h.update(format!("kosmos-link-v1\0{id}\0{kind}\0{target}").as_bytes());
            links.push(crate::types::ObjectLink {
                id: format!("lnk*{:x}", h.finalize()),
                source_object_id: id.into(),
                target_object_id: target,
                link_type: kind.into(),
                created_at: at.into(),
            });
        }
    }
    Ok(())
}
// Same write-target contract as `relation_with_aliases`.
pub fn relation_single_with_aliases(
    m: &Map<String, Value>,
    key: &str,
    aliases: &[&str],
    kind: &str,
    w: LinkWrite<'_>,
) -> Result<(), CompatFailure> {
    let LinkWrite {
        links,
        id,
        at,
        unknown: u,
    } = w;
    let pointer = || {
        if m.contains_key(key) {
            format!("/props/{key}")
        } else {
            aliases
                .iter()
                .find(|a| m.contains_key(**a))
                .map_or_else(|| format!("/props/{key}"), |a| format!("/props/{a}"))
        }
    };
    if let Some(v) = relation_val(m, key, aliases, u)? {
        if let Some(a) = v.as_array() {
            let mut ids = a.iter().filter_map(Value::as_str).collect::<Vec<_>>();
            ids.sort_unstable();
            ids.dedup();
            if ids.len() > 1 {
                return Err(invalid(&pointer()));
            }
        }
    }
    relation_with_aliases(m, key, aliases, kind, LinkWrite { links, id, at, unknown: u })
}
pub fn bundle(m: Map<String, Value>) -> Vec<LocalState> {
    if m.is_empty() {
        vec![]
    } else {
        vec![LocalState {
            data_json: Value::Object(m),
        }]
    }
}
pub fn qbundle(m: Map<String, Value>) -> Vec<Quarantine> {
    if m.is_empty() {
        vec![]
    } else {
        vec![Quarantine {
            fields_json: Value::Object(m),
        }]
    }
}
pub fn extensions(
    m: &Map<String, Value>,
    o: &mut Map<String, Value>,
    used: &std::collections::BTreeSet<String>,
    local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
) -> Result<(), CompatFailure> {
    let mut e = Map::new();
    for (k, v) in m {
        if used.contains(k) {
            continue;
        }
        if is_secret(k) {
            return Err(CompatFailure::SecretField {
                pointer: format!("/props/{k}"),
            });
        }
        if is_local(k) {
            local
                .entry("planning")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .map(|nested| nested.insert(camel(k), v.clone()));
        } else if is_quarantine(k) {
            q.insert(camel(k), v.clone());
        } else if !is_relation_id(k) {
            if let Some(v) = sanitize_extension(v, &camel(k), local, q) {
                e.insert(camel(k), v);
            }
        }
    }
    if let Some(Value::Object(x)) = o.get_mut("extensions") {
        x.insert("compatibility".into(), Value::Object(e));
    }
    Ok(())
}
fn sanitize_extension(
    v: &Value,
    namespace: &str,
    _local: &mut Map<String, Value>,
    q: &mut Map<String, Value>,
) -> Option<Value> {
    match v {
        Value::Object(m) => Some(Value::Object(
            m.iter()
                .filter_map(|(k, v)| {
                    if is_local(k) || is_quarantine(k) || is_relation_id(k) {
                        q.insert(format!("{namespace}.{}", camel(k)), v.clone());
                        None
                    } else {
                        sanitize_extension(v, &format!("{namespace}.{}", camel(k)), _local, q)
                            .map(|v| (camel(k), v))
                    }
                })
                .collect(),
        )),
        Value::Array(a) => Some(Value::Array(
            a.iter()
                .filter_map(|v| sanitize_extension(v, namespace, _local, q))
                .collect(),
        )),
        _ => Some(v.clone()),
    }
}
pub fn reject_secrets(m: &Map<String, Value>, p: &str) -> Result<(), CompatFailure> {
    for (k, v) in m {
        let pointer = format!("{p}/{k}");
        if is_secret(k) {
            return Err(CompatFailure::SecretField { pointer });
        }
        reject_secrets_value(v, &pointer)?;
    }
    Ok(())
}
fn reject_secrets_value(v: &Value, p: &str) -> Result<(), CompatFailure> {
    match v {
        Value::Object(m) => reject_secrets(m, p),
        Value::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                reject_secrets_value(item, &format!("{p}/{i}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn is_secret(k: &str) -> bool {
    let x = k.to_ascii_lowercase();
    x.contains("password")
        || x.contains("token")
        || x.contains("secret")
        || x.contains("credential")
        || x.contains("api_key")
        || x.contains("apikey")
}
fn is_local(k: &str) -> bool {
    matches!(
        k,
        "exe_path"
            | "exePath"
            | "install_dir"
            | "installDir"
            | "save_path"
            | "savePath"
            | "source_path"
            | "sourcePath"
            | "save_exists"
            | "saveExists"
            | "exe_name"
            | "exeName"
            | "installed"
            | "install_size_bytes"
            | "installSizeBytes"
            | "launch_pid"
            | "launchPid"
            | "window_id"
            | "windowId"
            | "process_state"
            | "processState"
            | "pid"
    )
}
fn is_quarantine(k: &str) -> bool {
    matches!(
        k,
        "source"
            | "source_app"
            | "sourceApp"
            | "source_app_id"
            | "sourceAppId"
            | "rawg_id"
            | "rawgId"
            | "provider"
            | "account_id"
            | "accountId"
            | "external_id"
            | "externalId"
            | "connector_id"
            | "connectorId"
            | "cover_image"
            | "coverImage"
            | "background_image"
            | "backgroundImage"
            | "project_id"
            | "projectId"
            | "workspace_id"
            | "workspaceId"
    )
}
fn is_relation_id(k: &str) -> bool {
    matches!(
        k,
        "project_id"
            | "projectId"
            | "tag_ids"
            | "tagIds"
            | "related_ids"
            | "relatedIds"
            | "source_note_id"
            | "sourceNoteId"
            | "photo_id"
            | "photoId"
            | "taskId"
            | "task_id"
            | "note_ids"
            | "noteIds"
            | "task_ids"
            | "taskIds"
            | "author_person_ids"
            | "authorPersonIds"
            | "related_notes"
    )
}
