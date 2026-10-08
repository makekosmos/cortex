//! Windows WASAPI capture — реализация `native_capture::start` для Windows.

use std::sync::mpsc;

use super::native_capture::{CapturedAudio, Session};

pub(super) fn start(
    capture_id: String,
    level_sink: Option<mpsc::Sender<f32>>,
) -> Result<(Session, u32, u16), String> {
    let (stop_tx, stop_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let join = std::thread::Builder::new()
        .name("mundus-dictation-capture".into())
        .spawn(move || {
            let result = capture_windows(stop_rx, ready_tx.clone(), level_sink);
            if let Err(error) = &result {
                let _ = ready_tx.send(Err(error.clone()));
            }
            result
        })
        .map_err(|_| "device_unavailable".to_string())?;
    let (sample_rate, channels) = ready_rx
        .recv()
        .map_err(|_| "device_unavailable".to_string())??;
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

/// RMS (0.0..=1.0) одного PCM16-фрейм-пакета — пилюля рисует из них waveform,
/// как Vue-пилюля из AnalyserNode. Молчание (SILENT flag) шлём как 0.
fn rms_i16le(bytes: &[u8]) -> f32 {
    let mut sum = 0.0f64;
    let mut n = 0u64;
    for chunk in bytes.as_chunks::<2>().0 {
        let s = i16::from_le_bytes(*chunk) as f64 / 32768.0;
        sum += s * s;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    (sum / n as f64).sqrt().min(1.0) as f32
}

/// Перцепционная нормализация уровня для waveform: линейный RMS → шкала
/// ~dBFS (-55 dB → 0, 0 dB → 1). Обычная речь (-30..-15 dBFS RMS) даёт
/// видимые бары — паритет с AnalyserNode-спектром Vue-пилюли, где даже
/// тихая речь двигала waveform.
fn level_for_ui(rms: f32) -> f32 {
    const FLOOR_DB: f32 = -55.0;
    let db = 20.0 * rms.max(1e-6).log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

fn capture_windows(
    stop_rx: mpsc::Receiver<()>,
    ready_tx: mpsc::Sender<Result<(u32, u16), String>>,
    level_sink: Option<mpsc::Sender<f32>>,
) -> Result<CapturedAudio, String> {
    use std::ptr::null_mut;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, WAVEFORMATEX,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    if unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_err() {
        let _ = ready_tx.send(Err("permission_denied".into()));
        return Err("permission_denied".into());
    }
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _com = ComGuard;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|_| "device_unavailable".to_string())?;
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }
        .map_err(|_| "device_unavailable".to_string())?;
    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }
        .map_err(|_| "permission_denied".to_string())?;
    let requested = WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 1,
        nSamplesPerSec: 16_000,
        nAvgBytesPerSec: 32_000,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
            10_000_000,
            0,
            &requested,
            None,
        )
    }
    .map_err(|_| "device_unavailable".to_string())?;
    let capture: IAudioCaptureClient =
        unsafe { client.GetService() }.map_err(|_| "device_unavailable".to_string())?;
    unsafe { client.Start() }.map_err(|_| "permission_denied".to_string())?;
    let _ = ready_tx.send(Ok((16_000, 1)));
    let mut pcm = Vec::new();
    // Level events throttled до ~30 Гц — UI-перерисовка чаще бессмысленна,
    // а broadcast-канал не должен засоряться.
    let mut last_level_emit = std::time::Instant::now() - std::time::Duration::from_millis(100);
    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }
        let mut packet_frames =
            unsafe { capture.GetNextPacketSize() }.map_err(|_| "capture_failed".to_string())?;
        while packet_frames > 0 {
            let mut data = null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            unsafe { capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }
                .map_err(|_| "capture_failed".to_string())?;
            let byte_count = frames as usize * 2;
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                pcm.resize(pcm.len() + byte_count, 0);
                if let Some(sink) = &level_sink {
                    if last_level_emit.elapsed() >= std::time::Duration::from_millis(30) {
                        last_level_emit = std::time::Instant::now();
                        let _ = sink.send(0.0);
                    }
                }
            } else if !data.is_null() {
                let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), byte_count) };
                if let Some(sink) = &level_sink {
                    if last_level_emit.elapsed() >= std::time::Duration::from_millis(30) {
                        last_level_emit = std::time::Instant::now();
                        let _ = sink.send(level_for_ui(rms_i16le(bytes)));
                    }
                }
                pcm.extend_from_slice(bytes);
            }
            unsafe { capture.ReleaseBuffer(frames) }.map_err(|_| "capture_failed".to_string())?;
            packet_frames =
                unsafe { capture.GetNextPacketSize() }.map_err(|_| "capture_failed".to_string())?;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = unsafe { client.Stop() };
    Ok(CapturedAudio {
        wav: wav_header(16_000, 1, 16, 2, pcm.len() as u32, &pcm),
        sample_rate: 16_000,
        channels: 1,
        duration_ms: pcm.len() as u64 * 1000 / 2 / 16_000,
        format: "wav",
    })
}

fn wav_header(
    sample_rate: u32,
    channels: u16,
    bits: u16,
    block_align: u16,
    data_len: u32,
    pcm: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32.saturating_add(data_len)).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&sample_rate.saturating_mul(block_align as u32).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}
