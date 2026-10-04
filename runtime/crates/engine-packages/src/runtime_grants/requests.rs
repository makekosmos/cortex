use super::*;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataRequest {
    ReadObject {
        type_id: String,
        type_version: String,
        object_id: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
    },
    ListObjects {
        type_id: String,
        type_version: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
        cursor: Option<String>,
        limit: u16,
    },
    CreateObject {
        type_id: String,
        type_version: String,
        object_id: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    UpdateObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    DeleteObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
    },
    Subscribe {
        type_id: String,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        versions: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        fields: Vec<String>,
        #[serde(deserialize_with = "deserialize_unique_strings")]
        relations: Vec<String>,
        cursor: Option<String>,
    },
    Link {
        source_type_id: String,
        source_version: String,
        source_object_id: String,
        relation: String,
        target_type_id: String,
        target_version: String,
        target_object_id: String,
        expected_hlc: Option<String>,
    },
    Batch {
        requests: Vec<BatchRequest>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FieldInput {
    pub field_id: String,
    pub value: FieldValue,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinkInput {
    pub relation_id: String,
    pub target_type_id: String,
    pub target_version: String,
    pub target_object_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<FieldValue>),
    Object(BTreeMap<String, FieldValue>),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BatchRequest {
    ReadObject {
        type_id: String,
        type_version: String,
        object_id: String,
        fields: Vec<String>,
        relations: Vec<String>,
    },
    ListObjects {
        type_id: String,
        type_version: String,
        fields: Vec<String>,
        relations: Vec<String>,
        cursor: Option<String>,
        limit: u16,
    },
    CreateObject {
        type_id: String,
        type_version: String,
        object_id: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    UpdateObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
        fields: Vec<FieldInput>,
        links: Vec<LinkInput>,
    },
    DeleteObject {
        type_id: String,
        type_version: String,
        object_id: String,
        expected_hlc: Option<String>,
    },
    Subscribe {
        type_id: String,
        versions: Vec<String>,
        fields: Vec<String>,
        relations: Vec<String>,
        cursor: Option<String>,
    },
    Link {
        source_type_id: String,
        source_version: String,
        source_object_id: String,
        relation: String,
        target_type_id: String,
        target_version: String,
        target_object_id: String,
        expected_hlc: Option<String>,
    },
}
impl DataRequest {
    pub fn validate(&self) -> Result<(), GrantError> {
        match self {
            Self::Batch { requests } => {
                if requests.is_empty() || requests.len() > MAX_BATCH {
                    return Err(GrantError::Contract("batch"));
                }
                requests.iter().try_for_each(BatchRequest::validate)
            }
            Self::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                if *limit == 0 || *limit > 500 {
                    return Err(GrantError::Contract("limit"));
                };
                if cursor.as_ref().is_some_and(|x| x.len() > 4096) {
                    return Err(GrantError::Contract("cursor"));
                };
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                if let Some(x) = object_id {
                    id(x, 128)?;
                }
                validate_fields(fields)?;
                validate_links(links)
            }
            Self::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                if expected_hlc.as_ref().is_some_and(|x| x.len() > 128) {
                    return Err(GrantError::Contract("hlc"));
                };
                validate_fields(fields)?;
                validate_links(links)
            }
            Self::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            } => {
                id(type_id, 128)?;
                id(type_version, 64)?;
                id(object_id, 128)?;
                if expected_hlc.as_ref().is_some_and(|x| x.len() > 128) {
                    return Err(GrantError::Contract("hlc"));
                };
                Ok(())
            }
            Self::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            } => {
                id(type_id, 128)?;
                if versions.is_empty() {
                    return Err(GrantError::Contract("versions"));
                };
                versions.iter().try_for_each(|x| {
                    Version::parse(x)
                        .map(|_| ())
                        .map_err(|_| GrantError::Contract("semver"))
                })?;
                if cursor.as_ref().is_some_and(|x| x.len() > 4096) {
                    return Err(GrantError::Contract("cursor"));
                };
                bounded_list(fields, MAX_FIELDS)?;
                bounded_list(relations, MAX_RELATIONS)
            }
            Self::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                ..
            } => {
                for x in [source_type_id, target_type_id] {
                    id(x, 128)?;
                }
                for x in [source_version, target_version] {
                    Version::parse(x).map_err(|_| GrantError::Contract("semver"))?;
                }
                for x in [source_object_id, target_object_id, relation] {
                    id(x, 128)?;
                }
                Ok(())
            }
        }
    }
}
fn validate_fields(fields: &[FieldInput]) -> Result<(), GrantError> {
    if fields.len() > MAX_FIELDS
        || !fields
            .iter()
            .map(|x| x.field_id.clone())
            .collect::<BTreeSet<_>>()
            .len()
            .eq(&fields.len())
    {
        return Err(GrantError::Contract("fields"));
    };
    for f in fields {
        id(&f.field_id, 128)?;
        validate_value(&f.value, 0, 0)?;
    }
    Ok(())
}
fn validate_links(links: &[LinkInput]) -> Result<(), GrantError> {
    if links.len() > MAX_RELATIONS {
        return Err(GrantError::Contract("links"));
    };
    let mut s = BTreeSet::new();
    for l in links {
        id(&l.relation_id, 128)?;
        if !s.insert(l.relation_id.clone()) {
            return Err(GrantError::Contract("duplicate link"));
        };
        id(&l.target_type_id, 128)?;
        Version::parse(&l.target_version).map_err(|_| GrantError::Contract("semver"))?;
        id(&l.target_object_id, 128)?;
    }
    Ok(())
}
fn validate_value(v: &FieldValue, depth: usize, nodes: usize) -> Result<(), GrantError> {
    if depth > 32 || nodes > 4096 {
        return Err(GrantError::Contract("value bounds"));
    };
    match v {
        FieldValue::Number(n) if !n.is_f64() && n.as_i64().is_none() && n.as_u64().is_none() => {
            return Err(GrantError::Contract("number"))
        }
        FieldValue::String(s) if s.len() > 65536 => return Err(GrantError::Contract("string")),
        FieldValue::Array(a) => {
            for x in a {
                validate_value(x, depth + 1, nodes + 1)?
            }
        }
        FieldValue::Object(o) => {
            for (k, x) in o {
                id(k, 65536)?;
                validate_value(x, depth + 1, nodes + 1)?
            }
        }
        _ => {}
    }
    Ok(())
}
impl BatchRequest {
    fn validate(&self) -> Result<(), GrantError> {
        DataRequest::from(self.clone()).validate()
    }
}
impl From<BatchRequest> for DataRequest {
    fn from(x: BatchRequest) -> Self {
        match x {
            BatchRequest::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            } => Self::ReadObject {
                type_id,
                type_version,
                object_id,
                fields,
                relations,
            },
            BatchRequest::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            } => Self::ListObjects {
                type_id,
                type_version,
                fields,
                relations,
                cursor,
                limit,
            },
            BatchRequest::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            } => Self::CreateObject {
                type_id,
                type_version,
                object_id,
                fields,
                links,
            },
            BatchRequest::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            } => Self::UpdateObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
                fields,
                links,
            },
            BatchRequest::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            } => Self::DeleteObject {
                type_id,
                type_version,
                object_id,
                expected_hlc,
            },
            BatchRequest::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            } => Self::Subscribe {
                type_id,
                versions,
                fields,
                relations,
                cursor,
            },
            BatchRequest::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                expected_hlc,
            } => Self::Link {
                source_type_id,
                source_version,
                source_object_id,
                relation,
                target_type_id,
                target_version,
                target_object_id,
                expected_hlc,
            },
        }
    }
}
