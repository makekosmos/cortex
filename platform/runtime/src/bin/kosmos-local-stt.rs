use kepler_backend::dictation::local_sidecar::run_stdio_service;

#[tokio::main]
async fn main() {
    if let Err(error) = run_stdio_service().await {
        eprintln!("[kosmos-local-stt] fatal: {error}");
        std::process::exit(1);
    }
}
