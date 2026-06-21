use std::env;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::Value;
use tokio::task::JoinHandle;

use super::local::{
    is_faster_whisper_engine, preload_with_whisper_backend_model,
    transcribe_with_whisper_backend_model, unload_whisper_backend, FasterWhisperWorker, LocalError,
    TranscriptionResult,
};
use super::local_sidecar_protocol::{
    LocalSttAccelerator, LocalSttAck, LocalSttModelSpec, LocalSttProfile, LocalSttRequest,
    LocalSttRequestEnvelope, LocalSttResponse, LocalSttResponseEnvelope, LocalSttStatus,
    LocalSttTranscription,
};
use super::local_whisper_dll::{whisper_dll_available, WhisperDllEngine};

#[derive(Default)]
struct WarmState {
    loaded_model: Option<LocalSttModelSpec>,
    backend: Option<String>,
    last_used_at: Option<Instant>,
    embedded: Option<WhisperDllEngine>,
    faster_whisper: Option<FasterWhisperWorker>,
}

pub struct LocalSttSidecarService {
    state: WarmState,
}

impl LocalSttSidecarService {
    pub fn new() -> Self {
        Self {
            state: WarmState::default(),
        }
    }

    pub async fn handle(&mut self, envelope: LocalSttRequestEnvelope) -> LocalSttResponseEnvelope {
        self.handle_with_cancel(envelope, None).await
    }

    async fn handle_with_cancel(
        &mut self,
        envelope: LocalSttRequestEnvelope,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> LocalSttResponseEnvelope {
        let request_id = envelope.request_id;
        match self.handle_request(envelope.request, cancel_flag).await {
            Ok(response) => LocalSttResponseEnvelope {
                request_id,
                ok: true,
                response: Some(response),
                error: None,
            },
            Err(error) => LocalSttResponseEnvelope {
                request_id,
                ok: false,
                response: None,
                error: Some(error.to_string()),
            },
        }
    }

    async fn handle_request(
        &mut self,
        request: LocalSttRequest,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<LocalSttResponse, LocalError> {
        self.unload_if_idle().await;
        match request {
            LocalSttRequest::Status => Ok(LocalSttResponse::Status(self.status())),
            LocalSttRequest::LoadModel { model } | LocalSttRequest::Preload { model } => {
                if is_faster_whisper_engine(&model.engine) {
                    #[cfg(not(test))]
                    {
                        self.load_faster_whisper_worker(&model).await?;
                    }
                    self.state.loaded_model = Some(model);
                    self.state.backend = Some("faster_whisper".into());
                    self.state.last_used_at = Some(Instant::now());
                    self.state.embedded = None;
                    return Ok(LocalSttResponse::Status(self.status()));
                }
                if let Err(error) = self.load_embedded_model(&model) {
                    eprintln!("[kosmos-local-stt] embedded whisper.dll preload fallback: {error}");
                    let warm = preload_with_whisper_backend_model(&model).await?;
                    self.state.loaded_model = warm.then_some(model);
                    self.state.backend = warm.then_some("whisper_server".into());
                    self.state.last_used_at = warm.then_some(Instant::now());
                    self.state.embedded = None;
                }
                Ok(LocalSttResponse::Status(self.status()))
            }
            LocalSttRequest::Transcribe {
                model,
                wav_base64,
                language,
                prompt,
            } => {
                let wav = base64::engine::general_purpose::STANDARD
                    .decode(wav_base64)
                    .map_err(|e| LocalError::CommandFailed(format!("invalid wav_base64: {e}")))?;
                let TranscriptionResult { text, backend } =
                    if is_faster_whisper_engine(&model.engine) {
                        self.transcribe_faster_whisper(
                            &model,
                            &wav,
                            &language,
                            &prompt,
                            cancel_flag.clone(),
                        )
                        .await?
                    } else {
                        match self.transcribe_embedded(
                            &model,
                            &wav,
                            &language,
                            &prompt,
                            cancel_flag.as_deref(),
                        ) {
                            Ok(result) => result,
                            Err(error) => {
                                if cancel_flag
                                    .as_ref()
                                    .is_some_and(|flag| flag.load(Ordering::SeqCst))
                                {
                                    return Err(error);
                                }
                                eprintln!(
                            "[kosmos-local-stt] embedded whisper.dll transcribe fallback: {error}"
                        );
                                let result = transcribe_with_whisper_backend_model(
                                    &model, &wav, &language, &prompt,
                                )
                                .await?;
                                self.state.embedded = None;
                                result
                            }
                        }
                    };

                if backend == "whisper_server"
                    || backend == "whisper_dll"
                    || backend == "faster_whisper"
                {
                    self.state.loaded_model = Some(model);
                    self.state.backend = Some(backend.clone());
                    self.state.last_used_at = Some(Instant::now());
                }

                Ok(LocalSttResponse::Transcription(LocalSttTranscription {
                    text,
                    backend,
                }))
            }
            LocalSttRequest::Cancel { .. } => {
                unload_whisper_backend().await;
                self.state = WarmState::default();
                Ok(LocalSttResponse::Ack(LocalSttAck {
                    accepted: true,
                    message: Some("cancelled".into()),
                }))
            }
            LocalSttRequest::Unload => {
                unload_whisper_backend().await;
                self.state = WarmState::default();
                Ok(LocalSttResponse::Status(self.status()))
            }
            LocalSttRequest::Shutdown => {
                unload_whisper_backend().await;
                self.state = WarmState::default();
                Ok(LocalSttResponse::Ack(LocalSttAck {
                    accepted: true,
                    message: Some("shutdown".into()),
                }))
            }
        }
    }

    fn status(&self) -> LocalSttStatus {
        let loaded_model = self.state.loaded_model.clone();
        LocalSttStatus {
            warm: loaded_model.is_some(),
            accelerator: loaded_model
                .as_ref()
                .map(|model| effective_accelerator(model))
                .unwrap_or_default(),
            device: loaded_model.as_ref().and_then(selected_device),
            profile: loaded_model
                .as_ref()
                .map(|model| model.profile.clone())
                .unwrap_or(LocalSttProfile::Fast),
            idle_unload_after_ms: loaded_model
                .as_ref()
                .and_then(|model| model.idle_unload_after_ms),
            loaded_model,
            backend: self.state.backend.clone(),
        }
    }

    fn load_embedded_model(&mut self, model: &LocalSttModelSpec) -> Result<(), LocalError> {
        if !whisper_dll_available(model) {
            return Err(LocalError::SidecarUnavailable(
                "whisper.dll is not available next to whisper command".into(),
            ));
        }

        let engine = WhisperDllEngine::load(model)?;
        self.state.loaded_model = Some(model.clone());
        self.state.backend = Some("whisper_dll".into());
        self.state.last_used_at = Some(Instant::now());
        self.state.embedded = Some(engine);
        Ok(())
    }

    fn transcribe_embedded(
        &mut self,
        model: &LocalSttModelSpec,
        wav: &[u8],
        language: &str,
        prompt: &str,
        cancel_flag: Option<&AtomicBool>,
    ) -> Result<TranscriptionResult, LocalError> {
        let current_model = self
            .state
            .embedded
            .as_ref()
            .map(|engine| engine.model_path());
        let requested_model = model.model_path.as_deref().map(Path::new);
        if self.state.embedded.is_none() || current_model != requested_model {
            self.load_embedded_model(model)?;
        }

        let engine = self.state.embedded.as_mut().ok_or_else(|| {
            LocalError::SidecarUnavailable("whisper.dll engine is not loaded".into())
        })?;
        engine.transcribe(model, wav, language, prompt, cancel_flag)
    }

    async fn load_faster_whisper_worker(
        &mut self,
        model: &LocalSttModelSpec,
    ) -> Result<(), LocalError> {
        let start = Instant::now();
        if self
            .state
            .faster_whisper
            .as_ref()
            .is_some_and(|worker| worker.matches_model(model))
        {
            eprintln!("[kosmos-local-stt] faster-whisper worker reused");
            return Ok(());
        }
        self.state.faster_whisper = None;
        let worker = FasterWhisperWorker::start(model).await?;
        eprintln!(
            "[kosmos-local-stt] faster-whisper worker loaded duration_ms={}",
            start.elapsed().as_millis()
        );
        self.state.faster_whisper = Some(worker);
        Ok(())
    }

    async fn transcribe_faster_whisper(
        &mut self,
        model: &LocalSttModelSpec,
        wav: &[u8],
        language: &str,
        prompt: &str,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<TranscriptionResult, LocalError> {
        let start = Instant::now();
        self.load_faster_whisper_worker(model).await?;
        let load_ms = start.elapsed().as_millis();
        let worker = self.state.faster_whisper.as_mut().ok_or_else(|| {
            LocalError::SidecarUnavailable("faster-whisper worker is not loaded".into())
        })?;
        match worker.transcribe(wav, language, prompt, cancel_flag).await {
            Ok(result) => {
                eprintln!(
                    "[kosmos-local-stt] faster-whisper transcribe done load_ms={} total_ms={}",
                    load_ms,
                    start.elapsed().as_millis()
                );
                Ok(result)
            }
            Err(error) => {
                if should_drop_faster_whisper_worker(&error) {
                    self.state.faster_whisper = None;
                }
                Err(error)
            }
        }
    }

    async fn unload_if_idle(&mut self) {
        let Some(model) = self.state.loaded_model.as_ref() else {
            return;
        };
        let Some(timeout_ms) = model.idle_unload_after_ms else {
            return;
        };
        let Some(last_used_at) = self.state.last_used_at else {
            return;
        };
        if last_used_at.elapsed() >= Duration::from_millis(timeout_ms) {
            unload_whisper_backend().await;
            self.state = WarmState::default();
        }
    }

    fn reset(&mut self) {
        self.state = WarmState::default();
    }
}

fn should_drop_faster_whisper_worker(error: &LocalError) -> bool {
    match error {
        LocalError::EmptyTranscript => false,
        LocalError::CommandFailed(message)
            if message.contains("cancelled") || message.contains("EmptyTranscript") =>
        {
            false
        }
        _ => true,
    }
}

fn idle_watch_interval() -> Duration {
    env::var("KOSMOS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(10))
}

fn spawn_idle_unload_watcher(
    service: Arc<tokio::sync::Mutex<LocalSttSidecarService>>,
    shutdown_signal: Arc<AtomicBool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let interval = idle_watch_interval();
        loop {
            tokio::time::sleep(interval).await;
            if shutdown_signal.load(Ordering::Relaxed) {
                break;
            }
            service.lock().await.unload_if_idle().await;
        }
    })
}

struct ActiveSidecarRequest {
    request_id: u64,
    kind: ActiveSidecarRequestKind,
    handle: JoinHandle<()>,
    cancel_flag: Arc<AtomicBool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveSidecarRequestKind {
    Preload,
    Transcribe,
}

fn send_envelope(
    tx: &mpsc::Sender<LocalSttResponseEnvelope>,
    envelope: LocalSttResponseEnvelope,
) -> io::Result<()> {
    tx.send(envelope)
        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "sidecar writer stopped"))
}

fn effective_accelerator(model: &LocalSttModelSpec) -> LocalSttAccelerator {
    match model.accelerator {
        LocalSttAccelerator::Cpu => LocalSttAccelerator::Cpu,
        LocalSttAccelerator::Gpu => LocalSttAccelerator::Gpu,
        LocalSttAccelerator::Auto => {
            if model
                .command_path
                .as_deref()
                .is_some_and(|path| path.to_ascii_lowercase().contains("cublas"))
            {
                LocalSttAccelerator::Gpu
            } else {
                LocalSttAccelerator::Auto
            }
        }
    }
}

fn selected_device(model: &LocalSttModelSpec) -> Option<String> {
    if !matches!(effective_accelerator(model), LocalSttAccelerator::Gpu) {
        return None;
    }
    Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .and_then(|text| text.lines().next().map(str::trim).map(str::to_owned))
                    .filter(|text| !text.is_empty())
            } else {
                None
            }
        })
}

pub async fn run_stdio_service() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let service = Arc::new(tokio::sync::Mutex::new(LocalSttSidecarService::new()));
    let shutdown_signal = Arc::new(AtomicBool::new(false));
    let idle_watcher =
        spawn_idle_unload_watcher(Arc::clone(&service), Arc::clone(&shutdown_signal));
    let active: Arc<tokio::sync::Mutex<Option<ActiveSidecarRequest>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let (tx, rx) = mpsc::channel::<LocalSttResponseEnvelope>();
    let writer_thread = std::thread::spawn(move || -> io::Result<()> {
        let mut writer = stdout.lock();
        for response in rx {
            serde_json::to_writer(&mut writer, &response)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
        Ok(())
    });

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim().trim_start_matches('\u{feff}');
        if trimmed.is_empty() {
            continue;
        }

        let envelope = match serde_json::from_str::<LocalSttRequestEnvelope>(trimmed) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[kosmos-local-stt] invalid request: {error}");
                if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(request_id) = value
                        .get("requestId")
                        .or_else(|| value.get("request_id"))
                        .and_then(Value::as_u64)
                    {
                        send_envelope(
                            &tx,
                            LocalSttResponseEnvelope {
                                request_id,
                                ok: false,
                                response: None,
                                error: Some(format!("invalid request: {error}")),
                            },
                        )?;
                    }
                }
                continue;
            }
        };
        let should_exit = matches!(envelope.request, LocalSttRequest::Shutdown);
        let request_id = envelope.request_id;
        match envelope.request {
            LocalSttRequest::Preload { .. } | LocalSttRequest::Transcribe { .. } => {
                let kind = match envelope.request {
                    LocalSttRequest::Preload { .. } => ActiveSidecarRequestKind::Preload,
                    LocalSttRequest::Transcribe { .. } => ActiveSidecarRequestKind::Transcribe,
                    _ => unreachable!(),
                };
                let service = Arc::clone(&service);
                let tx_task = tx.clone();
                let active_task = Arc::clone(&active);
                let cancel_flag = Arc::new(AtomicBool::new(false));
                let task_cancel_flag = Arc::clone(&cancel_flag);
                let handle = tokio::spawn(async move {
                    let response = service
                        .lock()
                        .await
                        .handle_with_cancel(envelope, Some(task_cancel_flag))
                        .await;
                    let _ = tx_task.send(response);
                    let mut active = active_task.lock().await;
                    if active
                        .as_ref()
                        .is_some_and(|active| active.request_id == request_id)
                    {
                        *active = None;
                    }
                });
                let mut active_guard = active.lock().await;
                let previous = active_guard.take();
                if let Some(previous) = previous {
                    if previous.kind == ActiveSidecarRequestKind::Preload
                        && kind == ActiveSidecarRequestKind::Transcribe
                    {
                        // Keep the recording-start warm-up alive. The transcribe task will wait
                        // on the service mutex and reuse the warmed faster-whisper worker instead
                        // of cancelling preload and paying cold-start latency on short utterances.
                    } else {
                        previous.cancel_flag.store(true, Ordering::SeqCst);
                        previous.handle.abort();
                    }
                }
                *active_guard = Some(ActiveSidecarRequest {
                    request_id,
                    kind,
                    handle,
                    cancel_flag,
                });
            }
            LocalSttRequest::Cancel { .. } => {
                let cancelled_active = if let Some(previous) = active.lock().await.take() {
                    previous.cancel_flag.store(true, Ordering::SeqCst);
                    previous.handle.abort();
                    true
                } else {
                    false
                };
                if cancelled_active {
                    unload_whisper_backend().await;
                    let service_reset = Arc::clone(&service);
                    tokio::spawn(async move {
                        service_reset.lock().await.reset();
                    });
                }
                send_envelope(
                    &tx,
                    LocalSttResponseEnvelope {
                        request_id,
                        ok: true,
                        response: Some(LocalSttResponse::Ack(LocalSttAck {
                            accepted: true,
                            message: Some(if cancelled_active {
                                "cancelled".into()
                            } else {
                                "nothing to cancel".into()
                            }),
                        })),
                        error: None,
                    },
                )?;
            }
            LocalSttRequest::Shutdown => {
                if let Some(previous) = active.lock().await.take() {
                    previous.cancel_flag.store(true, Ordering::SeqCst);
                    previous.handle.abort();
                }
                let response = service
                    .lock()
                    .await
                    .handle(LocalSttRequestEnvelope {
                        request_id,
                        request: LocalSttRequest::Shutdown,
                    })
                    .await;
                send_envelope(&tx, response)?;
            }
            _ => {
                let response = service.lock().await.handle(envelope).await;
                send_envelope(&tx, response)?;
            }
        }
        if should_exit {
            break;
        }
    }

    if let Some(previous) = active.lock().await.take() {
        previous.cancel_flag.store(true, Ordering::SeqCst);
        previous.handle.abort();
    }
    shutdown_signal.store(true, Ordering::Relaxed);
    idle_watcher.abort();
    drop(tx);
    match writer_thread.join() {
        Ok(result) => result?,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "sidecar writer panicked",
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_SIDECAR_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn warm_test_service(timeout_ms: u64) -> LocalSttSidecarService {
        let mut service = LocalSttSidecarService::new();
        service.state = WarmState {
            loaded_model: Some(LocalSttModelSpec {
                engine: "whisper.cpp".into(),
                model_id: Some("turbo".into()),
                model_path: Some("C:/models/turbo.bin".into()),
                command_path: Some("C:/tools/whisper.cpp-cublas/Release/whisper-cli.exe".into()),
                accelerator: LocalSttAccelerator::Gpu,
                profile: LocalSttProfile::Accurate,
                idle_unload_after_ms: Some(timeout_ms),
            }),
            backend: Some("whisper_server".into()),
            last_used_at: Some(Instant::now()),
            embedded: None,
            faster_whisper: None,
        };
        service
    }

    #[tokio::test]
    async fn status_exposes_accelerator_device_profile_and_idle_timeout() {
        let mut service = warm_test_service(300_000);
        let response = service
            .handle(LocalSttRequestEnvelope {
                request_id: 1,
                request: LocalSttRequest::Status,
            })
            .await;

        let Some(LocalSttResponse::Status(status)) = response.response else {
            panic!("expected status response: {response:?}");
        };
        assert!(status.warm);
        assert_eq!(status.accelerator, LocalSttAccelerator::Gpu);
        assert_eq!(status.profile, LocalSttProfile::Accurate);
        assert_eq!(status.idle_unload_after_ms, Some(300_000));
    }

    #[tokio::test]
    async fn cancel_unloads_backend_and_reports_accepted() {
        let mut service = warm_test_service(300_000);
        let response = service
            .handle(LocalSttRequestEnvelope {
                request_id: 2,
                request: LocalSttRequest::Cancel {
                    target_request_id: None,
                },
            })
            .await;

        let Some(LocalSttResponse::Ack(ack)) = response.response else {
            panic!("expected ack response: {response:?}");
        };
        assert!(ack.accepted);
        assert!(!service.status().warm);
    }

    #[tokio::test]
    async fn faster_whisper_preload_marks_warm_without_whisper_cpp_backend() {
        let mut service = LocalSttSidecarService::new();
        let response = service
            .handle(LocalSttRequestEnvelope {
                request_id: 4,
                request: LocalSttRequest::Preload {
                    model: LocalSttModelSpec {
                        engine: "faster-whisper".into(),
                        model_id: Some("turbo".into()),
                        model_path: Some("C:/models/faster-whisper-large-v3-turbo".into()),
                        command_path: None,
                        accelerator: LocalSttAccelerator::Gpu,
                        profile: LocalSttProfile::Fast,
                        idle_unload_after_ms: Some(300_000),
                    },
                },
            })
            .await;

        let Some(LocalSttResponse::Status(status)) = response.response else {
            panic!("expected status response: {response:?}");
        };
        assert!(status.warm);
        assert_eq!(status.backend.as_deref(), Some("faster_whisper"));
        assert_eq!(
            status
                .loaded_model
                .as_ref()
                .map(|model| model.engine.as_str()),
            Some("faster-whisper")
        );
    }

    #[test]
    fn faster_whisper_empty_transcript_does_not_drop_worker() {
        assert!(!should_drop_faster_whisper_worker(
            &LocalError::EmptyTranscript
        ));
        assert!(!should_drop_faster_whisper_worker(
            &LocalError::CommandFailed("faster-whisper cancelled".into())
        ));
        assert!(should_drop_faster_whisper_worker(
            &LocalError::CommandFailed("faster-whisper worker timed out".into())
        ));
    }

    #[tokio::test]
    async fn idle_watcher_unloads_without_new_request() {
        let _guard = ENV_SIDECAR_LOCK.lock().await;
        env::set_var("KOSMOS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS", "10");
        let service = Arc::new(tokio::sync::Mutex::new(warm_test_service(1)));
        {
            let mut service = service.lock().await;
            service.state.last_used_at = Some(Instant::now() - Duration::from_millis(20));
        }
        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let watcher = spawn_idle_unload_watcher(Arc::clone(&service), Arc::clone(&shutdown_signal));

        tokio::time::sleep(Duration::from_millis(50)).await;

        shutdown_signal.store(true, Ordering::Relaxed);
        watcher.abort();
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS");
        assert!(!service.lock().await.status().warm);
    }

    #[tokio::test]
    async fn status_unloads_after_idle_timeout() {
        let mut service = warm_test_service(1);
        service.state.last_used_at = Some(Instant::now() - Duration::from_millis(5));

        let response = service
            .handle(LocalSttRequestEnvelope {
                request_id: 3,
                request: LocalSttRequest::Status,
            })
            .await;

        let Some(LocalSttResponse::Status(status)) = response.response else {
            panic!("expected status response: {response:?}");
        };
        assert!(!status.warm);
        assert!(status.loaded_model.is_none());
    }
}
