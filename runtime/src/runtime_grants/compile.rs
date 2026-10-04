use super::*;
use crate::package_manifest::{DataAction, ManifestV2};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisteredType {
    pub type_id: String,
    pub version: String,
    pub fields: Vec<String>,
    pub relations: Vec<String>,
}
impl RegisteredType {
    pub fn new(id: &str, v: &str, f: &[&str], r: &[&str]) -> Self {
        Self {
            type_id: id.into(),
            version: v.into(),
            fields: f.iter().map(|x| (*x).into()).collect(),
            relations: r.iter().map(|x| (*x).into()).collect(),
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct RegistrySnapshot {
    pub types: Vec<RegisteredType>,
}
impl RegistrySnapshot {
    pub fn new(types: Vec<RegisteredType>) -> Self {
        Self { types }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrantRule {
    pub type_id: String,
    pub versions: Vec<String>,
    pub actions: BTreeSet<String>,
    pub fields_read: Vec<String>,
    pub fields_write: Vec<String>,
    pub relations_read: Vec<String>,
    pub relations_write: Vec<String>,
}
impl GrantRule {
    pub fn read(id: &str, v: &str, f: &[&str], r: &[&str]) -> Self {
        Self {
            type_id: id.into(),
            versions: vec![v.into()],
            actions: ["read".into()].into_iter().collect(),
            fields_read: f.iter().map(|x| (*x).into()).collect(),
            fields_write: vec![],
            relations_read: r.iter().map(|x| (*x).into()).collect(),
            relations_write: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompileInput {
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub rules: Vec<GrantRule>,
}
impl CompileInput {
    pub fn new(p: &str, v: &str, d: &str, rules: Vec<GrantRule>) -> Self {
        Self {
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            rules,
        }
    }
}

/// Compiles the typed capability directly from the validated v2 manifest's
/// canonical data access section. The registry remains authoritative for the
/// definition/version/field/relation surface; manifest payloads never supply
/// a grant at request time.
pub fn compile_manifest_v2(
    manifest: &ManifestV2,
    registry: &RegistrySnapshot,
    manifest_digest: &str,
) -> Result<LaunchGrant, GrantError> {
    let rules = manifest
        .data
        .access
        .iter()
        .map(|access| {
            let matching = registry
                .types
                .iter()
                .filter(|registered| {
                    registered.type_id == access.type_id
                        && Version::parse(&registered.version)
                            .ok()
                            .zip(VersionReq::parse(&access.versions).ok())
                            .is_some_and(|(version, requirement)| requirement.matches(&version))
                })
                .collect::<Vec<_>>();
            let fields_read = expand_scope(&access.fields.read, &matching, |registered| {
                &registered.fields
            })?;
            let fields_write = expand_scope(&access.fields.write, &matching, |registered| {
                &registered.fields
            })?;
            let relations_read = access
                .relations
                .as_ref()
                .map(|relations| {
                    expand_scope(&relations.read, &matching, |registered| {
                        &registered.relations
                    })
                })
                .transpose()?
                .unwrap_or_default();
            let relations_write = access
                .relations
                .as_ref()
                .map(|relations| {
                    expand_scope(&relations.write, &matching, |registered| {
                        &registered.relations
                    })
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GrantRule {
                type_id: access.type_id.clone(),
                versions: vec![access.versions.clone()],
                actions: access
                    .actions
                    .iter()
                    .map(|action| match action {
                        DataAction::Read => "read",
                        DataAction::Create => "create",
                        DataAction::Update => "update",
                        DataAction::Delete => "delete",
                        DataAction::Subscribe => "subscribe",
                        DataAction::Link => "link",
                    })
                    .map(str::to_owned)
                    .collect(),
                fields_read,
                fields_write,
                relations_read,
                relations_write,
            })
        })
        .collect::<Result<Vec<_>, GrantError>>()?;
    let mut grant = GrantCompiler::compile(
        CompileInput::new(&manifest.id, &manifest.version, manifest_digest, rules),
        registry,
    )?;
    let dictation_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if dictation_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !dictation_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Dictation {
            operations: dictation_operations,
        });
    }
    let focus_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if focus_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !focus_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Focus {
            operations: focus_operations,
        });
    }
    let agents_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if agents_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !agents_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Agents {
            operations: agents_operations,
        });
    }
    let worker_operations: Vec<String> = manifest
        .permissions
        .iter()
        .filter(|permission| permission.capability == "worker.invoke")
        .flat_map(|permission| permission.scopes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !worker_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::WorkerInvoke {
            operations: worker_operations,
        });
    }
    let app_network_scopes: Vec<String> = manifest
        .permissions
        .iter()
        .filter(|permission| permission.capability == "network")
        .flat_map(|permission| permission.scopes.iter().cloned())
        .filter(|scope| APP_NETWORK_SCOPES.contains(&scope.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !app_network_scopes.is_empty() {
        grant.capabilities.push(ScopedCapability::AppNetwork {
            scopes: app_network_scopes,
        });
    }
    let filesystem_operations: Vec<String> = manifest
        .permissions
        .iter()
        .flat_map(|permission| {
            permission.scopes.iter().filter_map(move |scope| {
                if filesystem_operation_capability(scope.as_str())
                    == Some(permission.capability.as_str())
                {
                    Some(scope.clone())
                } else {
                    None
                }
            })
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !filesystem_operations.is_empty() {
        grant.capabilities.push(ScopedCapability::Filesystem {
            operations: filesystem_operations,
        });
    }
    Ok(grant)
}

fn expand_scope<'a, F>(
    patterns: &[String],
    matching: &[&'a RegisteredType],
    values: F,
) -> Result<Vec<String>, GrantError>
where
    F: Fn(&'a RegisteredType) -> &'a [String],
{
    let available = matching
        .iter()
        .flat_map(|registered| values(registered).iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut expanded = BTreeSet::new();
    for pattern in patterns {
        if pattern == "*" || pattern.ends_with(".*") {
            let prefix = pattern.strip_suffix(".*").unwrap_or_default();
            let matches = available
                .iter()
                .filter(|value| prefix.is_empty() || value.starts_with(&format!("{prefix}.")))
                .cloned()
                .collect::<Vec<_>>();
            if matches.is_empty() && pattern != "*" {
                return Err(GrantError::Contract("wildcard registry"));
            }
            expanded.extend(matches);
        } else {
            expanded.insert(pattern.clone());
        }
    }
    Ok(expanded.into_iter().collect())
}

pub struct GrantCompiler;
impl GrantCompiler {
    pub fn compile(i: CompileInput, r: &RegistrySnapshot) -> Result<LaunchGrant, GrantError> {
        if i.rules.len() > MAX_RULES {
            return Err(GrantError::Contract("rules"));
        };
        Version::parse(&i.package_version).map_err(|_| GrantError::Contract("package semver"))?;
        let mut map: BTreeMap<String, GrantRule> = BTreeMap::new();
        for mut x in i.rules {
            id(&x.type_id, 128)?;
            if x.versions.is_empty() {
                return Err(GrantError::Contract("versions"));
            };
            let mut matching = Vec::new();
            for v in &x.versions {
                let req = VersionReq::parse(v)
                    .map_err(|_| GrantError::Contract("version requirement"))?;
                let versions = r
                    .types
                    .iter()
                    .filter(|t| {
                        t.type_id == x.type_id
                            && Version::parse(&t.version)
                                .ok()
                                .is_some_and(|registered| req.matches(&registered))
                    })
                    .collect::<Vec<_>>();
                if versions.is_empty() {
                    return Err(GrantError::Contract("registry version"));
                }
                matching.extend(versions);
            }
            bounded_list(&x.fields_read, MAX_FIELDS)?;
            bounded_list(&x.fields_write, MAX_FIELDS)?;
            bounded_list(&x.relations_read, MAX_RELATIONS)?;
            bounded_list(&x.relations_write, MAX_RELATIONS)?;
            for reg in matching {
                if x.fields_read
                    .iter()
                    .chain(x.fields_write.iter())
                    .any(|f| !reg.fields.contains(f))
                {
                    return Err(GrantError::Contract("field registry"));
                };
                if x.relations_read
                    .iter()
                    .chain(x.relations_write.iter())
                    .any(|f| !reg.relations.contains(f))
                {
                    return Err(GrantError::Contract("relation registry"));
                };
            }
            if let Some(old) = map.get_mut(&x.type_id) {
                old.versions.retain(|v| x.versions.contains(v));
                old.actions = old.actions.intersection(&x.actions).cloned().collect();
                old.fields_read.retain(|v| x.fields_read.contains(v));
                old.fields_write.retain(|v| x.fields_write.contains(v));
                old.relations_read.retain(|v| x.relations_read.contains(v));
                old.relations_write
                    .retain(|v| x.relations_write.contains(v));
            } else {
                x.versions.sort();
                x.fields_read.sort();
                x.fields_write.sort();
                x.relations_read.sort();
                x.relations_write.sort();
                map.insert(x.type_id.clone(), x);
            }
        }
        if map
            .values()
            .any(|x| x.versions.is_empty() || x.actions.is_empty())
        {
            return Err(GrantError::Contract("empty intersection"));
        };
        Ok(LaunchGrant {
            package_id: i.package_id,
            package_version: i.package_version,
            manifest_digest: i.manifest_digest,
            rules: map.into_values().collect(),
            capabilities: vec![],
        })
    }
}
