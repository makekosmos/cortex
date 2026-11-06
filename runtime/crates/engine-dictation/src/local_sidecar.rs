#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use base64::Engine;

use super::local::{
    preload_with_whisper_backend_model, transcribe_with_whisper_backend_model,
    unload_whisper_backend, LocalError, TranscriptionResult,
};
use super::local_sidecar_protocol::{
    LocalSttAccelerator, LocalSttAck, LocalSttModelSpec, LocalSttProfile, LocalSttRequest,
    LocalSttRequestEnvelope, LocalSttResponse, LocalSttResponseEnvelope, LocalSttStatus,
    LocalSttTranscription,
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
use super::local_whisper_dll::{whisper_dll_available, WhisperDllEngine};

mod stdio;
pub use stdio::run_stdio_service;

#[derive(Default)]
struct WarmState {
    loaded_model: Option<LocalSttModelSpec>,
    backend: Option<String>,
    last_used_at: Option<Instant>,
    embedded: Option<WhisperDllEngine>,
}

pub struct LocalSttSidecarService {
    state: WarmState,
}
impl Default for LocalSttSidecarService {
    fn default() -> Self {
        Self::new()
    }
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

    pub(super) async fn handle_with_cancel(
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
                if let Err(error) = self.load_embedded_model(&model) {
                    eprintln!("[mundus-local-stt] embedded whisper.dll preload fallback: {error}");
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
                let TranscriptionResult { text, backend } = match self.transcribe_embedded(
                    &model,
                    &wav,
                    &language,
                    &prompt,
                    cancel_flag.as_deref(),
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        if matches!(error, LocalError::EmptyTranscript) {
                            return Err(error);
                        }
                        if cancel_flag
                            .as_ref()
                            .is_some_and(|flag| flag.load(Ordering::SeqCst))
                        {
                            return Err(error);
                        }
                        eprintln!(
                            "[mundus-local-stt] embedded whisper.dll transcribe fallback: {error}"
                        );
                        let result =
                            transcribe_with_whisper_backend_model(&model, &wav, &language, &prompt)
                                .await?;
                        self.state.embedded = None;
                        result
                    }
                };

                if backend == "whisper_server" || backend == "whisper_dll" {
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
                .map(effective_accelerator)
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

    pub(super) async fn unload_if_idle(&mut self) {
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

    pub(super) fn reset(&mut self) {
        self.state = WarmState::default();
    }
}

fn idle_watch_interval() -> Duration {
    std::env::var("MUNDUS_LOCAL_STT_IDLE_WATCH_INTERVAL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(10))
}

fn spawn_idle_unload_watcher(
    service: Arc<tokio::sync::Mutex<LocalSttSidecarService>>,
    shutdown_signal: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
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

pub(super) fn effective_accelerator(model: &LocalSttModelSpec) -> LocalSttAccelerator {
    match model.accelerator {
        LocalSttAccelerator::Cpu => LocalSttAccelerator::Cpu,
        LocalSttAccelerator::Gpu => LocalSttAccelerator::Gpu,
        LocalSttAccelerator::Auto => LocalSttAccelerator::Auto,
    }
}

pub(super) fn selected_device(model: &LocalSttModelSpec) -> Option<String> {
    if !matches!(effective_accelerator(model), LocalSttAccelerator::Gpu) {
        return None;
    }
    let mut command = Command::new("nvidia-smi");
    command.args(["--query-gpu=name", "--format=csv,noheader"]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command.output().ok().and_then(|output| {
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
#[cfg(test)]
#[path = "local_sidecar/tests.rs"]
mod tests;
