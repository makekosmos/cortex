use std::sync::Mutex;

const DUCKED_VOLUME: f32 = 0.2;

static ORIGINAL_VOLUME: Mutex<Option<f32>> = Mutex::new(None);

pub fn duck_if_enabled(enabled: bool) {
    if !enabled {
        return;
    }
    if let Err(e) = duck() {
        eprintln!("[dictation::audio_duck] duck failed: {e}");
    }
}

pub fn restore() {
    if let Err(e) = restore_inner() {
        eprintln!("[dictation::audio_duck] restore failed: {e}");
    }
}

#[cfg(not(windows))]
fn duck() -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
fn restore_inner() -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn duck() -> Result<(), String> {
    let mut original = ORIGINAL_VOLUME.lock().unwrap_or_else(|e| e.into_inner());
    if original.is_some() {
        return Ok(());
    }
    let current = with_endpoint_volume(|volume| {
        let current = unsafe { volume.GetMasterVolumeLevelScalar() }.map_err(|e| e.to_string())?;
        unsafe {
            volume
                .SetMasterVolumeLevelScalar(DUCKED_VOLUME, std::ptr::null())
                .map_err(|e| e.to_string())?;
        }
        Ok(current)
    })?;
    *original = Some(current);
    Ok(())
}

#[cfg(windows)]
fn restore_inner() -> Result<(), String> {
    let mut original = ORIGINAL_VOLUME.lock().unwrap_or_else(|e| e.into_inner());
    let Some(level) = *original else {
        return Ok(());
    };
    with_endpoint_volume(|volume| unsafe {
        volume
            .SetMasterVolumeLevelScalar(level, std::ptr::null())
            .map_err(|e| e.to_string())
    })?;
    *original = None;
    Ok(())
}

#[cfg(windows)]
fn with_endpoint_volume<T>(
    f: impl FnOnce(&windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume) -> Result<T, String>,
) -> Result<T, String> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    // ponytail: no persistent COM apartment; reopen for rare start/stop calls.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_err() {
        return Err(format!("CoInitializeEx failed: {hr:?}"));
    }
    struct ComGuard;
    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
    let _guard = ComGuard;

    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| e.to_string())?;
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|e| e.to_string())?;
    let volume = unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| e.to_string())?;
    f(&volume)
}
