use super::*;

pub fn process_mode(args: impl IntoIterator<Item = String>) -> ProcessMode {
    let mut mode = ProcessMode::Supervisor;
    for arg in args {
        match arg.as_str() {
            CORE_WORKER_ARG => return ProcessMode::CoreWorker,
            RESTART_CORE_ARG => return ProcessMode::RestartCore,
            SHUTDOWN_ARG => return ProcessMode::Shutdown,
            START_ARG => mode = ProcessMode::Supervisor,
            _ => {}
        }
    }
    mode
}
