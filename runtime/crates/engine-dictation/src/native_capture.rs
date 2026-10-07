//! Engine-owned microphone capture. Workers receive only completed WAV data.

#[cfg(target_os = "macos")]
use base64::Engine;
use std::sync::mpsc;

pub struct CapturedAudio {
    pub wav: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
    pub format: &'static str,
}

/// Per-capture file source (e2e): helper decodes this file instead of the
/// mic. Per-request env on the spawned helper — the Engine env is never
/// allowed to hijack real capture (`env_remove` below).
#[derive(Debug)]
pub struct CaptureSource {
    pub path: std::path::PathBuf,
    pub speed: f32,
}

pub struct Session {
    pub(crate) capture_id: String,
    pub(crate) stop: mpsc::Sender<()>,
    pub(crate) join: std::thread::JoinHandle<Result<CapturedAudio, String>>,
}

pub fn start(
    device_id: Option<&str>,
    capture_id: String,
    level_sink: Option<mpsc::Sender<f32>>,
    pcm_sink: Option<mpsc::Sender<Vec<i16>>>,
    source: Option<CaptureSource>,
) -> Result<(Session, u32, u16), String> {
    if device_id.is_some_and(|id| !id.is_empty()) {
        return Err("device_unavailable".into());
    }
    #[cfg(windows)]
    {
        let _ = pcm_sink;
        if source.is_some() {
            return Err("file_source_unsupported".into());
        }
        return super::native_capture_windows::start(capture_id, level_sink);
    }
    #[cfg(target_os = "macos")]
    {
        return start_macos(capture_id, level_sink, pcm_sink, source);
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if source.is_some() {
            return Err("file_source_unsupported".into());
        }
        let _ = (capture_id, level_sink, pcm_sink);
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
    pcm_sink: Option<mpsc::Sender<Vec<i16>>>,
    source: Option<CaptureSource>,
) -> Result<(Session, u32, u16), String> {
    use std::io::Write;
    let helper = crate::macos_native::resolve_helper_pub("audio-capturer")
        .map_err(|e| format!("device_unavailable: {e}"))?;
    let mut cmd = std::process::Command::new(&helper);
    // Источник — только per-request: унаследованный env Engine'а не может
    // угнать реальный микрофон.
    cmd.env_remove("MUNDUS_DICTATION_CAPTURE_FILE")
        .env_remove("MUNDUS_DICTATION_CAPTURE_FILE_SPEED");
    if let Some(source) = &source {
        cmd.env("MUNDUS_DICTATION_CAPTURE_FILE", &source.path).env(
            "MUNDUS_DICTATION_CAPTURE_FILE_SPEED",
            source.speed.to_string(),
        );
    }
    let mut child = cmd
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
            let result = capture_macos(
                &mut stdin,
                stdout,
                stop_rx,
                level_sink,
                pcm_sink,
                ready_tx.clone(),
            );
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
    /// Блок PCM-сэмплов из `drain` + сколько сэмплов потеряно переполнением.
    Pcm(Vec<i16>, u64),
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

/// 16kHz mono PCM s16 → канонический 44-байт WAV (тот же формат, что пишет
/// helper и pill renderer).
#[cfg(any(target_os = "macos", test))]
pub(crate) fn wav_from_pcm16(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}

/// Потребители событий помпы: уровни в pill waveform, pcm-блоки в
/// накопитель полной записи и в streaming-транскрибёр.
#[cfg(target_os = "macos")]
struct CaptureSinks<'a> {
    level: Option<&'a mpsc::Sender<f32>>,
    pcm: Option<&'a mpsc::Sender<Vec<i16>>>,
    acc: Vec<i16>,
    saw_pcm: bool,
    dropped: u64,
}

#[cfg(target_os = "macos")]
impl CaptureSinks<'_> {
    fn accept(&mut self, ev: CaptureEvent) {
        match ev {
            CaptureEvent::Meter(l) => {
                if let Some(tx) = self.level {
                    let _ = tx.send(l);
                }
            }
            CaptureEvent::Pcm(block, dropped) => {
                self.acc.extend_from_slice(&block);
                self.dropped += dropped;
                self.saw_pcm = true;
                if let Some(tx) = self.pcm {
                    let _ = tx.send(block);
                }
            }
            _ => {}
        }
    }
}

/// Ждём нужного события с дедлайном; meter'ы и pcm-блоки попутно сливаем в
/// sink'и, посторонние события (готовые file/error) остаются фатальными.
#[cfg(target_os = "macos")]
fn wait_for_event(
    event_rx: &mpsc::Receiver<CaptureEvent>,
    sinks: &mut CaptureSinks<'_>,
    is: fn(&CaptureEvent) -> bool,
    timeout: std::time::Duration,
) -> Result<CaptureEvent, String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err("capture_failed: helper timeout".into());
        }
        match event_rx.recv_timeout(left) {
            Ok(ev) if is(&ev) => return Ok(ev),
            Ok(CaptureEvent::Error(e)) => return Err(format!("capture_failed: {e}")),
            Ok(ev) => sinks.accept(ev),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err("capture_failed: helper timeout".into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("capture_failed: helper died".into())
            }
        }
    }
}

/// Драйвер `audio-capturer` helper'а: warmup → start → meter × 50мс +
/// drain × 250мс → final drain → stop → WAV-файл (файл — только fallback
/// и для очистки; аудио собирается из drain-потока). Ответы helper'а —
/// JSON-строки на stdout; одна помпа-читалка раскладывает их по типам,
/// сессионный поток ждёт события через канал с таймаутами.
#[cfg(target_os = "macos")]
fn capture_macos(
    stdin: &mut impl std::io::Write,
    stdout: impl std::io::Read + Send + 'static,
    stop_rx: mpsc::Receiver<()>,
    level_sink: Option<mpsc::Sender<f32>>,
    pcm_sink: Option<mpsc::Sender<Vec<i16>>>,
    ready_tx: mpsc::Sender<Result<(u32, u16), String>>,
) -> Result<CapturedAudio, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::time::Duration;

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
            } else if v.get("pcm").is_some() {
                let pcm = v
                    .get("pcm")
                    .and_then(|p| p.as_str())
                    .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
                    .map(|bytes| {
                        bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| i16::from_le_bytes(*pair))
                            .collect::<Vec<i16>>()
                    })
                    .unwrap_or_default();
                let dropped = v.get("dropped").and_then(|d| d.as_u64()).unwrap_or(0);
                Some(CaptureEvent::Pcm(pcm, dropped))
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
    // Полная запись вне 30/60с ring'а helper'а: drain-блоки складываются в
    // `sinks.acc` и одновременно уходят в streaming-транскрибёр (pcm_sink).
    let mut sinks = CaptureSinks {
        level: level_sink.as_ref(),
        pcm: pcm_sink.as_ref(),
        acc: Vec::new(),
        saw_pcm: false,
        dropped: 0,
    };

    cmd(stdin, "warmup")?;
    tracing::info!("dictation: macOS capture warmup sent");
    wait_for_event(
        &event_rx,
        &mut sinks,
        |e| matches!(e, CaptureEvent::Ready),
        Duration::from_secs(15),
    )?;
    tracing::info!("dictation: macOS capture ready");
    cmd(stdin, "start")?;
    wait_for_event(
        &event_rx,
        &mut sinks,
        |e| matches!(e, CaptureEvent::Recording),
        Duration::from_secs(5),
    )?;
    tracing::info!("dictation: macOS capture recording");
    let _ = ready_tx.send(Ok((16_000, 1)));

    // Уровень для pill waveform — meter каждые 50мс (helper ограничивает
    // эмит сам на ~20 Гц). 20 Гц нужен живому отклику баров на голос.
    // Каждый 5-й опрос — ещё и drain: PCM поток для streaming-транскриба
    // и полной записи вне ring'а helper'а.
    let mut iteration = 0u32;
    while stop_rx.try_recv().is_err() {
        if cmd(stdin, "meter").is_err() {
            break;
        }
        if iteration % 5 == 4 && cmd(stdin, "drain").is_err() {
            break;
        }
        iteration += 1;
        // Между опросами сливаем meter/pcm-события в sink'и.
        while let Ok(ev) = event_rx.try_recv() {
            sinks.accept(ev);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Финальный drain ДО stop — забираем хвост записи, что ещё не уехал.
    if cmd(stdin, "drain").is_ok() {
        let _ = wait_for_event(
            &event_rx,
            &mut sinks,
            |e| matches!(e, CaptureEvent::Pcm(..)),
            Duration::from_secs(2),
        );
    }
    if sinks.dropped > 0 {
        tracing::warn!(
            dropped = sinks.dropped,
            "dictation: capture ring overflow — audio has a gap"
        );
    }

    cmd(stdin, "stop")?;
    let path = wait_for_event(
        &event_rx,
        &mut sinks,
        |e| matches!(e, CaptureEvent::File(_)),
        Duration::from_secs(10),
    )?
    .unwrap_file();
    let wav = if sinks.saw_pcm {
        wav_from_pcm16(&sinks.acc, 16_000)
    } else {
        // Старый helper без drain — файл из ring'а как раньше.
        std::fs::read(&path).map_err(|e| format!("capture_failed: {e}"))?
    };
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

#[cfg(test)]
mod tests {
    use super::wav_from_pcm16;

    #[test]
    fn wav_from_pcm16_roundtrips_through_wav_reader() {
        let samples: Vec<i16> = (-2000..2000).step_by(97).collect();
        let wav = wav_from_pcm16(&samples, 16_000);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        let data_size = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(data_size, samples.len() * 2);
        assert_eq!(wav.len(), 44 + data_size);

        let decoded: Vec<i16> = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn wav_from_pcm16_empty_produces_header_only() {
        let wav = wav_from_pcm16(&[], 16_000);
        assert_eq!(wav.len(), 44);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 0);
    }
}
