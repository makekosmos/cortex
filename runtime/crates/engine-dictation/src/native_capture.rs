//! Engine-owned microphone capture. Workers receive only completed WAV data.

use std::sync::mpsc;

pub struct CapturedAudio {
    pub wav: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub format: &'static str,
}

pub struct Session {
    pub(crate) capture_id: String,
    stop: mpsc::Sender<()>,
    join: std::thread::JoinHandle<Result<CapturedAudio, String>>,
}

pub fn start(
    device_id: Option<&str>,
    capture_id: String,
    level_sink: Option<mpsc::Sender<f32>>,
) -> Result<(Session, u32, u16), String> {
    if device_id.is_some_and(|id| !id.is_empty()) {
        return Err("device_unavailable".into());
    }
    #[cfg(windows)]
    {
        return start_windows(capture_id, level_sink);
    }
    #[cfg(target_os = "macos")]
    {
        return start_macos(capture_id, level_sink);
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = (capture_id, level_sink);
        Err("device_unavailable".into())
    }
}

pub fn stop(session: Session) -> Result<CapturedAudio, String> {
    let _ = session.stop.send(());
    session
        .join
        .join()
        .map_err(|_| "capture_failed".to_string())?
}

#[cfg(target_os = "macos")]
fn start_macos(
    capture_id: String,
    level_sink: Option<mpsc::Sender<f32>>,
) -> Result<(Session, u32, u16), String> {
    use std::io::Write;
    let helper = crate::macos_native::resolve_helper_pub("audio-capturer")
        .map_err(|e| format!("device_unavailable: {e}"))?;
    let mut child = std::process::Command::new(&helper)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("device_unavailable: spawn {e}"))?;
    tracing::info!(helper = %helper.display(), "dictation: audio-capturer spawned");
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "device_unavailable: no stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "device_unavailable: no stdout".to_string())?;
    let (stop_tx, stop_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let join = std::thread::Builder::new()
        .name("mundus-dictation-capture".into())
        .spawn(move || {
            let result = capture_macos(&mut stdin, stdout, stop_rx, level_sink, ready_tx.clone());
            if let Err(error) = &result {
                let _ = ready_tx.send(Err(error.clone()));
            }
            // "exit" для чистого shutdown helper'а — процесс доживать не должен.
            let _ = writeln!(stdin, "exit");
            let _ = child.wait();
            result
        })
        .map_err(|_| "device_unavailable".to_string())?;
    let (sample_rate, channels) = ready_rx
        .recv_timeout(std::time::Duration::from_secs(25))
        .map_err(|_| {
            tracing::warn!("dictation: macOS capture ready-wait timed out");
            "device_unavailable".to_string()
        })
        .and_then(|r| {
            r.inspect_err(|e| {
                tracing::warn!(error = %e, "dictation: macOS capture failed");
            })
        })?;
    Ok((
        Session {
            capture_id,
            stop: stop_tx,
            join,
        },
        sample_rate,
        channels,
    ))
}

/// События из stdout-помпы `audio-capturer` — handshake'и, файл результата
/// и meter-уровни.
#[cfg(target_os = "macos")]
enum CaptureEvent {
    Ready,
    Recording,
    File(String),
    Meter(f32),
    Error(String),
}

#[cfg(target_os = "macos")]
impl CaptureEvent {
    /// `wait_for` уже отфильтровал — других веток тут быть не может.
    fn unwrap_file(self) -> String {
        match self {
            CaptureEvent::File(p) => p,
            _ => String::new(),
        }
    }
}

/// Драйвер `audio-capturer` helper'а: warmup → start → meter × 100мс →
/// stop → WAV-файл. Ответы helper'а — JSON-строки на stdout; одна
/// помпа-читалка раскладывает их по типам, сессионный поток ждёт события
/// через канал с таймаутами.
#[cfg(target_os = "macos")]
fn capture_macos(
    stdin: &mut impl std::io::Write,
    stdout: impl std::io::Read + Send + 'static,
    stop_rx: mpsc::Receiver<()>,
    level_sink: Option<mpsc::Sender<f32>>,
    ready_tx: mpsc::Sender<Result<(u32, u16), String>>,
) -> Result<CapturedAudio, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::time::{Duration, Instant};

    let (event_tx, event_rx) = mpsc::channel::<CaptureEvent>();
    let pump = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let event = if v.get("ready").and_then(|b| b.as_bool()) == Some(true) {
                Some(CaptureEvent::Ready)
            } else if v.get("recording").and_then(|b| b.as_bool()) == Some(true) {
                Some(CaptureEvent::Recording)
            } else if let Some(path) = v.get("file").and_then(|f| f.as_str()) {
                Some(CaptureEvent::File(path.to_string()))
            } else if let Some(avg) = v
                .get("meter")
                .and_then(|m| m.get("average"))
                .and_then(|a| a.as_f64())
            {
                Some(CaptureEvent::Meter(avg as f32))
            } else {
                v.get("error")
                    .and_then(|e| e.as_str())
                    .map(|e| CaptureEvent::Error(e.to_string()))
            };
            if let Some(event) = event {
                if event_tx.send(event).is_err() {
                    break;
                }
            }
        }
    });

    // Протокол helper'а — JSON-строки `{"command":"<name>"}`.
    let cmd = |stdin: &mut dyn Write, c: &str| -> Result<(), String> {
        writeln!(stdin, "{{\"command\":\"{c}\"}}")
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("capture_failed: {e}"))
    };
    // Ждём нужного события с дедлайном; meter'ы попутно сливаем в sink,
    // посторонние события (готовые file/error) остаются фатальными.
    let wait_for =
        |is: fn(&CaptureEvent) -> bool, timeout: Duration| -> Result<CaptureEvent, String> {
            let deadline = Instant::now() + timeout;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err("capture_failed: helper timeout".into());
                }
                match event_rx.recv_timeout(left) {
                    Ok(ev) if is(&ev) => return Ok(ev),
                    Ok(CaptureEvent::Meter(l)) => {
                        if let Some(tx) = level_sink.as_ref() {
                            let _ = tx.send(l);
                        }
                    }
                    Ok(CaptureEvent::Error(e)) => return Err(format!("capture_failed: {e}")),
                    Ok(_) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return Err("capture_failed: helper timeout".into())
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("capture_failed: helper died".into())
                    }
                }
            }
        };

    cmd(stdin, "warmup")?;
    tracing::info!("dictation: macOS capture warmup sent");
    wait_for(
        |e| matches!(e, CaptureEvent::Ready),
        Duration::from_secs(15),
    )?;
    tracing::info!("dictation: macOS capture ready");
    cmd(stdin, "start")?;
    wait_for(
        |e| matches!(e, CaptureEvent::Recording),
        Duration::from_secs(5),
    )?;
    tracing::info!("dictation: macOS capture recording");
    let _ = ready_tx.send(Ok((16_000, 1)));

    // Уровень для pill waveform — meter каждые 100мс (helper ограничивает
    // эмит сам на ~10 Гц).
    while stop_rx.try_recv().is_err() {
        if cmd(stdin, "meter").is_err() {
            break;
        }
        // Между опросами сливаем meter-события в sink.
        while let Ok(ev) = event_rx.try_recv() {
            if let CaptureEvent::Meter(l) = ev {
                if let Some(tx) = level_sink.as_ref() {
                    let _ = tx.send(l);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    cmd(stdin, "stop")?;
    let path = wait_for(
        |e| matches!(e, CaptureEvent::File(_)),
        Duration::from_secs(10),
    )?
    .unwrap_file();
    let wav = std::fs::read(&path).map_err(|e| format!("capture_failed: {e}"))?;
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    // Pump join'ится только когда helper умрёт — шлём exit до join'а,
    // иначе stdout остаётся открытым и join висит навечно.
    let _ = cmd(stdin, "exit");
    let _ = pump.join();

    let frames = (wav.len().saturating_sub(44) / 2) as u64;
    Ok(CapturedAudio {
        wav,
        sample_rate: 16_000,
        channels: 1,
        duration_ms: frames * 1000 / 16_000,
        format: "wav",
    })
}
