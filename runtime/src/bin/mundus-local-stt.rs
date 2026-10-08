use engine_dictation::local_sidecar::run_stdio_service;

#[tokio::main]
async fn main() {
    if let Err(error) = run_stdio_service().await {
        eprintln!("[mundus-local-stt] fatal: {error}");
        std::process::exit(1);
    }
}
