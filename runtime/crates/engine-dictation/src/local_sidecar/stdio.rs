use std::io::{self, BufRead, Write};
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::task::JoinHandle;

use serde_json::Value;

use super::super::local::unload_whisper_backend;
use super::super::local_sidecar_protocol::{
    LocalSttAck, LocalSttRequest, LocalSttRequestEnvelope, LocalSttResponse,
    LocalSttResponseEnvelope,
};
use super::{spawn_idle_unload_watcher, LocalSttSidecarService};

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
                eprintln!("[mundus-local-stt] invalid request: {error}");
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
                let kind = if matches!(envelope.request, LocalSttRequest::Preload { .. }) {
                    ActiveSidecarRequestKind::Preload
                } else {
                    ActiveSidecarRequestKind::Transcribe
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
                        // on the service mutex and reuse the warmed whisper.cpp backend instead
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
        Err(_) => return Err(io::Error::other("sidecar writer panicked")),
    }
    Ok(())
}
