use super::*;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchGrant {
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub rules: Vec<GrantRule>,
    pub capabilities: Vec<ScopedCapability>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopedCapability {
    Storage {
        root_capability_id: String,
        actions: Vec<String>,
        path_patterns: Vec<String>,
        max_bytes: u64,
    },
    Network {
        origins: Vec<String>,
        methods: Vec<String>,
        max_request_bytes: u64,
        max_response_bytes: u64,
    },
    Dialog {
        kinds: Vec<String>,
    },
    Window {
        actions: Vec<String>,
    },
    Dictation {
        operations: Vec<String>,
    },
    Focus {
        operations: Vec<String>,
    },
    Agents {
        operations: Vec<String>,
    },
    WorkerInvoke {
        operations: Vec<String>,
    },
    /// Named `network` scopes (`bookMetadata`, `images`) — Engine-owned app
    /// network ops, distinct from origin-form worker network grants.
    AppNetwork {
        scopes: Vec<String>,
    },
    Filesystem {
        operations: Vec<String>,
    },
}

impl LaunchGrant {
    pub fn allows_dictation_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(
                capability,
                ScopedCapability::Dictation {
                    operations
                } if operations.iter().any(|allowed| allowed == operation),
            )
        })
    }

    pub fn allows_focus_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(
                capability,
                ScopedCapability::Focus {
                    operations
                } if operations.iter().any(|allowed| allowed == operation),
            )
        })
    }

    pub fn allows_agents_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(
                capability,
                ScopedCapability::Agents {
                    operations
                } if operations.iter().any(|allowed| allowed == operation),
            )
        })
    }

    pub fn allows_worker_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(capability, ScopedCapability::WorkerInvoke {
                operations
            } if operations.iter().any(|allowed| {
                allowed == operation || allowed.strip_suffix(
                    ".*"
                ).is_some_and(|prefix| operation.starts_with(&format!("{prefix}.")))
            }))
        })
    }

    pub fn allows_app_network_scope(&self, scope: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(
                capability,
                ScopedCapability::AppNetwork {
                    scopes
                } if scopes.iter().any(|allowed| allowed == scope),
            )
        })
    }

    pub fn allows_filesystem_operation(&self, operation: &str) -> bool {
        self.capabilities.iter().any(|capability| {
            matches!(
                capability,
                ScopedCapability::Filesystem {
                    operations
                } if operations.iter().any(|allowed| allowed == operation),
            )
        })
    }

    /// Authorize the canonical typed request against the grant compiled for
    /// this launch. Callers must invoke this before translating to legacy RPC.
    pub fn authorize_request(&self, request: &DataRequest) -> Result<(), GrantError> {
        request.validate()?;
        let (type_id, version, action, fields, relations): (
            &str,
            &str,
            &str,
            Vec<String>,
            Vec<String>,
        ) = match request {
            DataRequest::ReadObject {
                type_id,
                type_version,
                fields,
                relations,
                ..
            }
            | DataRequest::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                ..
            } => (
                type_id,
                type_version,
                "read",
                fields.clone(),
                relations.clone(),
            ),
            DataRequest::CreateObject {
                type_id,
                type_version,
                fields,
                links,
                ..
            } => (
                type_id,
                type_version,
                "create",
                fields.iter().map(|f| f.field_id.clone()).collect(),
                links.iter().map(|l| l.relation_id.clone()).collect(),
            ),
            DataRequest::UpdateObject {
                type_id,
                type_version,
                fields,
                links,
                ..
            } => (
                type_id,
                type_version,
                "update",
                fields.iter().map(|f| f.field_id.clone()).collect(),
                links.iter().map(|l| l.relation_id.clone()).collect(),
            ),
            DataRequest::DeleteObject {
                type_id,
                type_version,
                ..
            } => (type_id, type_version, "delete", vec![], vec![]),
            DataRequest::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                ..
            } => (
                type_id,
                versions.first().ok_or(GrantError::Missing)?,
                "subscribe",
                fields.clone(),
                relations.clone(),
            ),
            DataRequest::Link {
                source_type_id,
                source_version,
                relation,
                target_type_id,
                target_version,
                ..
            } => {
                let source = self
                    .rules
                    .iter()
                    .find(|r| {
                        r.type_id == *source_type_id
                            && r.versions.iter().any(|v| {
                                VersionReq::parse(v).ok().is_some_and(|req| {
                                    Version::parse(source_version)
                                        .ok()
                                        .is_some_and(|version| req.matches(&version))
                                })
                            })
                    })
                    .ok_or(GrantError::Missing)?;
                if !source.actions.contains("link")
                    || !source.relations_write.iter().any(|x| x == relation)
                {
                    return Err(GrantError::Missing);
                }
                let target = self
                    .rules
                    .iter()
                    .find(|r| {
                        r.type_id == *target_type_id
                            && r.versions.iter().any(|v| {
                                VersionReq::parse(v).ok().is_some_and(|req| {
                                    Version::parse(target_version)
                                        .ok()
                                        .is_some_and(|version| req.matches(&version))
                                })
                            })
                    })
                    .ok_or(GrantError::Missing)?;
                if !target.actions.contains("read") {
                    return Err(GrantError::Missing);
                }
                return Ok(());
            }
            DataRequest::Batch { requests } => {
                for item in requests {
                    self.authorize_request(&item.clone().into())?;
                }
                return Ok(());
            }
        };
        let version = Version::parse(version).map_err(|_| GrantError::Contract("semver"))?;
        let rule = self
            .rules
            .iter()
            .find(|r| {
                r.type_id == *type_id
                    && r.versions.iter().any(|v| {
                        VersionReq::parse(v)
                            .ok()
                            .is_some_and(|req| req.matches(&version))
                    })
            })
            .ok_or(GrantError::Missing)?;
        if !rule.actions.contains(action) {
            return Err(GrantError::Missing);
        }
        let allowed_fields = if matches!(action, "read" | "subscribe") {
            &rule.fields_read
        } else {
            &rule.fields_write
        };
        if fields
            .iter()
            .any(|field| !allowed_fields.iter().any(|allowed| allowed == field))
        {
            return Err(GrantError::Missing);
        }
        let allowed_relations = if matches!(action, "read" | "subscribe") {
            &rule.relations_read
        } else {
            &rule.relations_write
        };
        if relations
            .iter()
            .any(|relation| !allowed_relations.iter().any(|allowed| allowed == relation))
        {
            return Err(GrantError::Missing);
        }
        Ok(())
    }

    pub fn projection_json(&self) -> Result<String, GrantError> {
        serde_json::to_string(self).map_err(|_| GrantError::Contract("serialize"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeProjection {
    package_id: String,
    package_version: String,
    manifest_digest: String,
    capabilities: Vec<ScopedCapability>,
}
impl SafeProjection {
    pub fn from_parts(p: &str, v: &str, d: &str, c: Vec<ScopedCapability>) -> Self {
        Self {
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            capabilities: c,
        }
    }
    pub fn to_value(&self) -> Value {
        serde_json::json!(
            {"package_id":self.package_id,
            "package_version":self.package_version,
            "manifest_digest":self.manifest_digest,
            "capabilities":self.capabilities.iter().map(
                |c|match c{ScopedCapability::Storage{root_capability_id,
                actions,
                max_bytes,
                ..}=>serde_json::json!(
                    {"kind":"storage",
                    "root_capability_id":root_capability_id,
                    "actions":actions,
                    "max_bytes":max_bytes}),
                _=>serde_json::to_value(c).unwrap_or(Value::Null)},
            ).collect::<Vec<_>>()})
    }
}
