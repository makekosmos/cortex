use super::*;

impl PackageWorkerSupervisor {
    pub fn authorize(
        &self,
        id: &str,
        version: &str,
        token: &str,
        pid: u32,
        generation: u64,
        method: &WorkerMethod,
        scope: Option<&str>,
    ) -> bool {
        lock(&self.inner.workers)
            .get(&(id.into(), version.into()))
            .and_then(|w| w.grant.as_ref())
            .is_some_and(|g| {
                g.authorize(
                    token,
                    pid,
                    generation,
                    self.inner.api_major,
                    self.inner.api_major,
                    method,
                    scope,
                )
            })
    }

    pub fn timeout() -> Duration {
        Duration::from_secs(30)
    }

    pub(super) fn insert_failed(&self, key: (String, String)) {
        lock(&self.inner.workers).insert(key, failed_worker());
    }
}
