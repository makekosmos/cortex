/// A validated scope whose serialized JSON size can be estimated without
/// returning the underlying object fields.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DataVolumeScope {
    type_id: String,
    type_version: String,
    fields: Vec<String>,
}

impl DataVolumeScope {
    pub fn new(type_id: String, type_version: String, fields: Vec<String>) -> Result<Self, String> {
        if type_id.is_empty()
            || type_id.len() > 128
            || type_version.is_empty()
            || type_version.len() > 64
            || fields.len() > 256
            || fields.iter().any(|field| {
                field.len() > 128
                    || !(field == "title"
                        || field == "content"
                        || field
                            .strip_prefix("props.")
                            .is_some_and(|key| !key.is_empty()))
            })
        {
            return Err("invalid data volume scope".into());
        }

        Ok(Self {
            type_id,
            type_version,
            fields,
        })
    }

    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    pub fn type_version(&self) -> &str {
        &self.type_version
    }

    pub fn fields(&self) -> &[String] {
        &self.fields
    }
}

/// One request to estimate the transfer size for a set of object scopes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DataVolumeQuery {
    scopes: Vec<DataVolumeScope>,
}

impl DataVolumeQuery {
    pub fn new(scopes: Vec<DataVolumeScope>) -> Result<Self, String> {
        if scopes.len() > 256 {
            return Err("too many data volume scopes".into());
        }
        Ok(Self { scopes })
    }

    pub fn scopes(&self) -> &[DataVolumeScope] {
        &self.scopes
    }
}

/// The estimated JSON size visible for one exact type/version and field scope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DataVolumeEstimate {
    pub type_id: String,
    pub type_version: String,
    pub object_count: u64,
    pub estimated_json_bytes: u64,
}

/// Storage capability for calculating counts and visible JSON byte totals.
pub trait DataVolumeReadPort {
    fn estimate_data_volume(
        &self,
        query: &DataVolumeQuery,
    ) -> Result<Vec<DataVolumeEstimate>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_rejects_invalid_scope_and_excessive_scope_count() {
        let error = DataVolumeScope::new("".into(), "1.0.0".into(), vec![])
            .expect_err("empty type ids must be rejected");
        assert_eq!(error, "invalid data volume scope");

        let scopes = (0..257)
            .map(|_| {
                DataVolumeScope::new("note".into(), "1.0.0".into(), vec![]).expect("valid scope")
            })
            .collect();
        let error = DataVolumeQuery::new(scopes).expect_err("too many scopes must be rejected");
        assert_eq!(error, "too many data volume scopes");
    }
}
