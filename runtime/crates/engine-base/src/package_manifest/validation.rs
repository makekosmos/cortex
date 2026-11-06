use super::{
    DataAction, ManifestError, ManifestV2, PackageKind, PermissionRequest, TargetRuntime,
    MAX_ACCESS, MAX_DEFINES, MAX_DESCRIPTION, MAX_ENTRYPOINT, MAX_ICON, MAX_ID, MAX_MAPPINGS,
    MAX_NAME, MAX_PERMISSIONS, MAX_SCOPE, MAX_SCOPES, MAX_STORE_CATEGORIES,
    MAX_STORE_COMPATIBILITY, MAX_TARGETS,
};
use semver::{Version, VersionReq};
use std::collections::HashSet;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn validate_identity(
    schema_version: u32,
    id: &str,
    name: &str,
    version: &str,
    engine_api: &str,
    entrypoint: &str,
    publisher: &str,
    permissions: &[PermissionRequest],
) -> Result<(), ManifestError> {
    if !matches!(schema_version, 1 | 2) || !safe_package_id(id) {
        return Err(ManifestError::InvalidField("schema_version/id"));
    }
    if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(ManifestError::InvalidField("name"));
    }
    Version::parse(version).map_err(|_| ManifestError::InvalidField("version"))?;
    VersionReq::parse(engine_api).map_err(|_| ManifestError::InvalidField("engine_api"))?;
    if publisher != "kosmos" {
        return Err(ManifestError::InvalidField("publisher"));
    }
    if !safe_relative_path(entrypoint, MAX_ENTRYPOINT) {
        return Err(ManifestError::InvalidField("entrypoint"));
    }
    if permissions.len() > MAX_PERMISSIONS {
        return Err(ManifestError::InvalidField("permissions"));
    }
    let mut seen = HashSet::new();
    for p in permissions {
        if !known_capability(&p.capability) {
            return Err(ManifestError::UnknownCapability(p.capability.clone()));
        }
        if !seen.insert(&p.capability) {
            return Err(ManifestError::DuplicateCapability(p.capability.clone()));
        }
        if p.scopes.len() > MAX_SCOPES
            || p.scopes
                .iter()
                .any(|s| s.is_empty() || s.len() > MAX_SCOPE || s.contains('\0'))
            || (p.capability == "worker.invoke"
                && p.scopes.iter().any(|scope| !safe_operation_scope(scope)))
        {
            return Err(ManifestError::InvalidField("permissions.scopes"));
        }
    }
    Ok(())
}
pub fn validate_v2(m: &ManifestV2) -> Result<(), ManifestError> {
    if m.description
        .as_ref()
        .is_some_and(|description| description.len() > MAX_DESCRIPTION)
    {
        return Err(ManifestError::InvalidField("description"));
    }
    if m.icon.as_ref().is_some_and(|icon| !safe_icon_path(icon)) {
        return Err(ManifestError::InvalidField("icon"));
    }
    if m.targets.is_empty()
        || m.targets.len() > MAX_TARGETS
        || unique_len(&m.targets) != m.targets.len()
    {
        return Err(ManifestError::InvalidField("targets"));
    }
    for t in &m.targets {
        if t.os.is_empty() || t.os.len() > 8 || unique_len(&t.os) != t.os.len() {
            return Err(ManifestError::InvalidField("targets.os"));
        }
        if let Some(a) = &t.arch {
            if a.is_empty() || a.len() > 8 || unique_len(a) != a.len() {
                return Err(ManifestError::InvalidField("targets.arch"));
            }
        }
        if t.runtime == TargetRuntime::Worker {
            let entrypoint = t.entrypoint.as_deref().or_else(|| {
                matches!(m.kind, PackageKind::Source | PackageKind::Bridge)
                    .then_some(m.entrypoint.as_str())
            });
            if !entrypoint.is_some_and(|entrypoint| {
                safe_relative_path(entrypoint, MAX_ENTRYPOINT)
                    && entrypoint.to_ascii_lowercase().ends_with(".exe")
            }) {
                return Err(ManifestError::InvalidField("targets.entrypoint"));
            }
        } else if t.entrypoint.is_some() {
            return Err(ManifestError::InvalidField("targets.entrypoint"));
        }
    }
    for (index, left) in m.targets.iter().enumerate() {
        for right in &m.targets[index + 1..] {
            let os_overlap = left.os.iter().any(|os| right.os.contains(os));
            let arch_overlap = match (&left.arch, &right.arch) {
                (Some(left), Some(right)) => left.iter().any(|arch| right.contains(arch)),
                _ => true,
            };
            if left.runtime == right.runtime && os_overlap && arch_overlap {
                return Err(ManifestError::InvalidField("targets"));
            }
        }
    }
    if m.data.access.len() > MAX_ACCESS
        || m.data.defines.len() > MAX_DEFINES
        || m.data.mappings.len() > MAX_MAPPINGS
    {
        return Err(ManifestError::InvalidField("data"));
    }
    let mut types = HashSet::new();
    for r in &m.data.access {
        if !valid_type_id(&r.type_id)
            || r.type_id.len() > 128
            || !types.insert(&r.type_id)
            || r.actions.is_empty()
            || r.actions.len() > 6
            || unique_len(&r.actions) != r.actions.len()
        {
            return Err(ManifestError::InvalidField("data.access"));
        }
        if r.versions.len() > 128 {
            return Err(ManifestError::InvalidField("data.access.versions"));
        }
        VersionReq::parse(&r.versions)
            .map_err(|_| ManifestError::InvalidField("data.access.versions"))?;
        validate_lists(&r.fields.read, 128, true, "data.access.fields.read")?;
        validate_lists(&r.fields.write, 128, false, "data.access.fields.write")?;
        if !r.fields.read.is_empty() && !r.actions.contains(&DataAction::Read) {
            return Err(ManifestError::InvalidField("data.access.fields.read"));
        }
        if !r.fields.write.is_empty()
            && !r
                .actions
                .iter()
                .any(|a| matches!(a, DataAction::Create | DataAction::Update))
        {
            return Err(ManifestError::InvalidField("data.access.fields.write"));
        }
        if let Some(rel) = &r.relations {
            validate_lists(&rel.read, 64, true, "data.access.relations.read")?;
            validate_lists(&rel.write, 64, false, "data.access.relations.write")?;
            if !rel.read.is_empty() && !r.actions.contains(&DataAction::Read) {
                return Err(ManifestError::InvalidField("data.access.relations.read"));
            }
            if !rel.write.is_empty() && !r.actions.contains(&DataAction::Link) {
                return Err(ManifestError::InvalidField("data.access.relations.write"));
            }
        }
    }
    let mut defs = HashSet::new();
    for d in &m.data.defines {
        let prefix = format!("{}.", m.id);
        let suffix = d.type_id.strip_prefix(&prefix).unwrap_or("");
        if suffix.is_empty()
            || suffix.len() > 64
            || suffix.split('.').any(|part| part.is_empty())
            || !d.type_id.starts_with(&prefix)
            || !valid_type_id(&d.type_id)
            || Version::parse(&d.version).is_err()
            || !defs.insert((&d.type_id, &d.version))
            || !safe_relative_path(&d.schema, 256)
            || d.content_contract
                .as_ref()
                .is_some_and(|p| !safe_relative_path(p, 256))
            || d.relations
                .as_ref()
                .is_some_and(|p| !safe_relative_path(p, 256))
        {
            return Err(ManifestError::InvalidField("data.defines"));
        }
    }
    let mut mappings = HashSet::new();
    for x in &m.data.mappings {
        if !valid_type_id(&x.type_id)
            || x.versions.len() > 128
            || VersionReq::parse(&x.versions).is_err()
            || !mappings.insert(&x.type_id)
        {
            return Err(ManifestError::InvalidField("data.mappings"));
        }
    }
    if let Some(integration) = &m.integration {
        integration.validate(m)?;
    }
    if let Some(store) = &m.store {
        if store.description.as_ref().is_some_and(|d| {
            d.is_empty() || d.len() > MAX_DESCRIPTION || d.chars().any(char::is_control)
        }) || store.categories.len() > MAX_STORE_CATEGORIES
            || unique_len(&store.categories) != store.categories.len()
            || store.categories.iter().any(|c| {
                c.is_empty()
                    || c.len() > 64
                    || !c
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            || store.connects_to.as_ref().is_some_and(|id| {
                id.len() > 128 || !id.starts_with("external.") || !valid_type_id(id)
            })
            || store.data_compatibility.len() > MAX_STORE_COMPATIBILITY
        {
            return Err(ManifestError::InvalidField("store"));
        }
        for row in &store.data_compatibility {
            if !valid_type_id(&row.type_id)
                || row.type_id.len() > 128
                || row.versions.len() > 128
                || VersionReq::parse(&row.versions).is_err()
                || row.roles.is_empty()
                || !valid_type_id(&row.via)
            {
                return Err(ManifestError::InvalidField("store.data_compatibility"));
            }
        }
    }
    Ok(())
}
fn validate_lists(
    xs: &[String],
    max: usize,
    read: bool,
    field: &'static str,
) -> Result<(), ManifestError> {
    if xs.len() > max
        || unique_len(xs) != xs.len()
        || xs
            .iter()
            .any(|x| x.is_empty() || x.chars().any(char::is_control) || (!read && x == "*"))
    {
        return Err(ManifestError::InvalidField(field));
    }
    Ok(())
}
fn unique_len<T: Eq + std::hash::Hash>(xs: &[T]) -> usize {
    xs.iter().collect::<HashSet<_>>().len()
}
fn safe_relative_path(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !Path::new(value).is_absolute()
        && !value.contains('\\')
        && !value.contains('\0')
        && value
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn safe_icon_path(value: &str) -> bool {
    safe_relative_path(value, MAX_ICON)
        && value.rsplit_once('.').is_some_and(|(_, extension)| {
            extension.eq_ignore_ascii_case("ico") || extension.eq_ignore_ascii_case("png")
        })
}
fn valid_type_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|p| {
            !p.is_empty()
                && p.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-')
                })
        })
}
fn known_capability(value: &str) -> bool {
    matches!(
        value,
        "ark.read"
            | "ark.write"
            | "launcher.search"
            | "network"
            | "filesystem.read"
            | "filesystem.write"
            | "clipboard"
            | "notifications"
            | "process.spawn"
            | "worker.invoke"
            | "dictation.control"
    )
}
fn safe_operation_scope(scope: &str) -> bool {
    fn valid_byte(byte: u8) -> bool {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    }
    let operation = scope.strip_suffix(".*").unwrap_or(scope);
    !operation.is_empty() && operation.len() <= 128 && operation.bytes().all(valid_byte)
}
fn safe_package_id(value: &str) -> bool {
    value.len() <= MAX_ID
        && valid_type_id(value)
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && !matches!(
            value.split('.').next().unwrap_or_default(),
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
        )
}
