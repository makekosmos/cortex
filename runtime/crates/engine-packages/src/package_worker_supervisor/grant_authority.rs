use super::*;

impl PackageWorkerSupervisor {
    pub fn bind_grant_authority(&self, grants: Arc<GrantAuthorityRegistry>) {
        *lock(&self.inner.grants) = Some(grants);
    }
}
