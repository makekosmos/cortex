/// Receiver-owned policy identity exchanged on the sync wire.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PolicyIdentity {
    pub revision: i64,
    pub digest: String,
    pub active: bool,
}

impl PolicyIdentity {
    pub fn new(revision: i64, digest: impl Into<String>, active: bool) -> Result<Self, String> {
        let digest = digest.into();
        if revision < 0
            || digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid_sync_policy_identity".into());
        }
        Ok(Self {
            revision,
            digest,
            active,
        })
    }
}
