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
    stop: mpsc::Sender<()>,
    join: std::thread::JoinHandle<Result<CapturedAudio, String>>,
}

pub fn start(device_id: Option<&str>) -> Result<(Session, u32, u16), String> {
    if device_id.is_some_and(|id| !id.is_empty()) {
        return Err("device_unavailable".into());
    }
    #[cfg(windows)]
    {
        return start_windows();
    }
    #[cfg(not(windows))]
    {
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

#[cfg(windows)]
fn start_windows() -> Result<(Session, u32, u16), String> {
    let (stop_tx, stop_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::channel();
    let join = std::thread::Builder::new()
        .name("kosmos-dictation-capture".into())
        .spawn(move || {
            let result = capture_windows(stop_rx, ready_tx.clone());
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
            stop: stop_tx,
            join,
        },
        sample_rate,
        channels,
    ))
}

#[cfg(windows)]
fn capture_windows(
    stop_rx: mpsc::Receiver<()>,
    ready_tx: mpsc::Sender<Result<(u32, u16), String>>,
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
            } else if !data.is_null() {
                pcm.extend_from_slice(unsafe {
                    std::slice::from_raw_parts(data.cast::<u8>(), byte_count)
                });
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

#[cfg(test)]
mod tests {
    use super::wav_header;

    #[test]
    fn synthetic_wav_header_is_well_formed() {
        let wav = wav_header(16_000, 1, 16, 2, 4, &[1, 2, 3, 4]);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 48);
    }
}
