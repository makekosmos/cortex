use super::*;

impl PackageWorkerSupervisor {
    pub(super) fn start_windows_boxed<'a>(
        &'a self,
        key: (String, String),
        spec: LaunchSpec,
        generation: u64,
        restart_count: u32,
        failure_streak: u8,
        restart_allowed: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send + 'a>> {
        Box::pin(self.start_windows(
            key,
            spec,
            generation,
            restart_count,
            failure_streak,
            restart_allowed,
        ))
    }
}
