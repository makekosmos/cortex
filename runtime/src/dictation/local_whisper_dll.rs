use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::Library;

use super::local::{clean_whisper_transcript, LocalError, TranscriptionResult};
use super::local_sidecar_protocol::{LocalSttAccelerator, LocalSttModelSpec, LocalSttProfile};

const WHISPER_SAMPLING_GREEDY: WhisperSamplingStrategy = 0;
const WHISPER_SAMPLING_BEAM_SEARCH: WhisperSamplingStrategy = 1;

type WhisperSamplingStrategy = c_uint;
type GgmlAbortCallback = Option<unsafe extern "C" fn(data: *mut c_void) -> bool>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperAHead {
    n_text_layer: c_int,
    n_head: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperAHeads {
    n_heads: usize,
    heads: *const WhisperAHead,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperContextParams {
    use_gpu: bool,
    flash_attn: bool,
    gpu_device: c_int,
    dtw_token_timestamps: bool,
    dtw_aheads_preset: c_uint,
    dtw_n_top: c_int,
    dtw_aheads: WhisperAHeads,
    dtw_mem_size: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperVadParams {
    threshold: f32,
    min_speech_duration_ms: c_int,
    min_silence_duration_ms: c_int,
    max_speech_duration_s: f32,
    speech_pad_ms: c_int,
    samples_overlap: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperGreedyParams {
    best_of: c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperBeamSearchParams {
    beam_size: c_int,
    patience: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct WhisperFullParams {
    strategy: WhisperSamplingStrategy,
    n_threads: c_int,
    n_max_text_ctx: c_int,
    offset_ms: c_int,
    duration_ms: c_int,
    translate: bool,
    no_context: bool,
    no_timestamps: bool,
    single_segment: bool,
    print_special: bool,
    print_progress: bool,
    print_realtime: bool,
    print_timestamps: bool,
    token_timestamps: bool,
    thold_pt: f32,
    thold_ptsum: f32,
    max_len: c_int,
    split_on_word: bool,
    max_tokens: c_int,
    debug_mode: bool,
    audio_ctx: c_int,
    tdrz_enable: bool,
    suppress_regex: *const c_char,
    initial_prompt: *const c_char,
    carry_initial_prompt: bool,
    prompt_tokens: *const c_int,
    prompt_n_tokens: c_int,
    language: *const c_char,
    detect_language: bool,
    suppress_blank: bool,
    suppress_nst: bool,
    temperature: f32,
    max_initial_ts: f32,
    length_penalty: f32,
    temperature_inc: f32,
    entropy_thold: f32,
    logprob_thold: f32,
    no_speech_thold: f32,
    greedy: WhisperGreedyParams,
    beam_search: WhisperBeamSearchParams,
    new_segment_callback: *mut c_void,
    new_segment_callback_user_data: *mut c_void,
    progress_callback: *mut c_void,
    progress_callback_user_data: *mut c_void,
    encoder_begin_callback: *mut c_void,
    encoder_begin_callback_user_data: *mut c_void,
    abort_callback: GgmlAbortCallback,
    abort_callback_user_data: *mut c_void,
    logits_filter_callback: *mut c_void,
    logits_filter_callback_user_data: *mut c_void,
    grammar_rules: *mut *const c_void,
    n_grammar_rules: usize,
    i_start_rule: usize,
    grammar_penalty: f32,
    vad: bool,
    vad_model_path: *const c_char,
    vad_params: WhisperVadParams,
}

type WhisperContextDefaultParams = unsafe extern "C" fn() -> WhisperContextParams;
type WhisperFullDefaultParams = unsafe extern "C" fn(WhisperSamplingStrategy) -> WhisperFullParams;
type WhisperInitFromFileWithParams =
    unsafe extern "C" fn(*const c_char, WhisperContextParams) -> *mut c_void;
type WhisperFree = unsafe extern "C" fn(*mut c_void);
type WhisperFull = unsafe extern "C" fn(*mut c_void, WhisperFullParams, *const f32, c_int) -> c_int;
type WhisperFullNSegments = unsafe extern "C" fn(*mut c_void) -> c_int;
type WhisperFullGetSegmentText = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;
type GgmlBackendLoadAllFromPath = unsafe extern "C" fn(*const c_char);

#[derive(Clone, Copy)]
struct WhisperApi {
    context_default_params: WhisperContextDefaultParams,
    full_default_params: WhisperFullDefaultParams,
    init_from_file_with_params: WhisperInitFromFileWithParams,
    free: WhisperFree,
    full: WhisperFull,
    full_n_segments: WhisperFullNSegments,
    full_get_segment_text: WhisperFullGetSegmentText,
}

pub(crate) struct WhisperDllEngine {
    _library: Library,
    _ggml_library: Option<Library>,
    api: WhisperApi,
    ctx: *mut c_void,
    model_path: PathBuf,
}

unsafe impl Send for WhisperDllEngine {}

impl Drop for WhisperDllEngine {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { (self.api.free)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}

impl WhisperDllEngine {
    pub(crate) fn load(model: &LocalSttModelSpec) -> Result<Self, LocalError> {
        let model_path = PathBuf::from(
            model
                .model_path
                .as_deref()
                .ok_or(LocalError::MissingModelPath)?,
        );
        let command_path = PathBuf::from(
            model
                .command_path
                .as_deref()
                .ok_or(LocalError::MissingCommandPath)?,
        );
        let dll_path = fs::canonicalize(whisper_dll_path(&command_path)?).map_err(|e| {
            LocalError::SidecarUnavailable(format!("failed to canonicalize whisper.dll path: {e}"))
        })?;
        let dll_dir = dll_path.parent().ok_or_else(|| {
            LocalError::SidecarUnavailable(format!(
                "whisper.dll has no parent directory: {}",
                dll_path.display()
            ))
        })?;

        let library = unsafe { load_library(&dll_path) }.map_err(|e| {
            LocalError::SidecarUnavailable(format!(
                "failed to load whisper.dll at {}: {e}",
                dll_path.display()
            ))
        })?;
        let api = unsafe { load_api(&library)? };
        let ggml_library = load_ggml_backends(dll_dir)?;
        let model_c = cstring_path(&model_path)?;
        let mut params = unsafe { (api.context_default_params)() };
        params.use_gpu = !matches!(model.accelerator, LocalSttAccelerator::Cpu);
        params.gpu_device = 0;

        let ctx = unsafe { (api.init_from_file_with_params)(model_c.as_ptr(), params) };
        if ctx.is_null() {
            return Err(LocalError::CommandFailed(format!(
                "whisper.dll failed to load model {}",
                model_path.display()
            )));
        }

        Ok(Self {
            _library: library,
            _ggml_library: ggml_library,
            api,
            ctx,
            model_path,
        })
    }

    pub(crate) fn model_path(&self) -> &Path {
        &self.model_path
    }

    pub(crate) fn transcribe(
        &mut self,
        model: &LocalSttModelSpec,
        wav_bytes: &[u8],
        language: &str,
        prompt: &str,
        cancel_flag: Option<&AtomicBool>,
    ) -> Result<TranscriptionResult, LocalError> {
        let samples = wav_pcm16_to_f32_16k_mono(wav_bytes)?;
        let strategy = match model.profile {
            LocalSttProfile::Fast => WHISPER_SAMPLING_GREEDY,
            LocalSttProfile::Accurate => WHISPER_SAMPLING_BEAM_SEARCH,
        };
        let mut params = unsafe { (self.api.full_default_params)(strategy) };
        params.n_threads = super::local::local_whisper_threads()
            .try_into()
            .unwrap_or(c_int::MAX);
        params.no_timestamps = true;
        params.print_special = false;
        params.print_progress = false;
        params.print_realtime = false;
        params.print_timestamps = false;
        params.temperature = 0.0;
        params.temperature_inc = 0.2;
        params.no_speech_thold = 0.6;
        match model.profile {
            LocalSttProfile::Fast => {
                params.greedy.best_of = 1;
            }
            LocalSttProfile::Accurate => {
                params.beam_search.beam_size = 5;
                params.beam_search.patience = -1.0;
            }
        }

        let language_c = super::local::whisper_language_arg(language)
            .map(CString::new)
            .transpose()
            .map_err(|e| LocalError::CommandFailed(format!("invalid language: {e}")))?;
        if let Some(language_c) = language_c.as_ref() {
            params.language = language_c.as_ptr();
        }

        let prompt_c = if prompt.trim().is_empty() {
            None
        } else {
            Some(
                CString::new(prompt.trim())
                    .map_err(|e| LocalError::CommandFailed(format!("invalid prompt: {e}")))?,
            )
        };
        if let Some(prompt_c) = prompt_c.as_ref() {
            params.initial_prompt = prompt_c.as_ptr();
        }
        let vad_model_c = whisper_vad_model_path(&model.command_path)
            .map(|path| cstring_path(&path))
            .transpose()?;
        if let Some(vad_model_c) = vad_model_c.as_ref() {
            params.vad = true;
            params.vad_model_path = vad_model_c.as_ptr();
        }
        if let Some(cancel_flag) = cancel_flag {
            params.abort_callback = Some(whisper_abort_callback);
            params.abort_callback_user_data =
                (cancel_flag as *const AtomicBool).cast::<c_void>() as *mut c_void;
        }

        let rc = unsafe {
            (self.api.full)(
                self.ctx,
                params,
                samples.as_ptr(),
                samples.len().try_into().unwrap_or(c_int::MAX),
            )
        };
        if rc != 0 {
            return Err(LocalError::CommandFailed(format!(
                "whisper.dll inference failed with code {rc}"
            )));
        }

        let n_segments = unsafe { (self.api.full_n_segments)(self.ctx) };
        let mut text = String::new();
        for i in 0..n_segments {
            let ptr = unsafe { (self.api.full_get_segment_text)(self.ctx, i) };
            if !ptr.is_null() {
                let segment = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
                text.push_str(&segment);
                text.push('\n');
            }
        }
        let text = clean_whisper_transcript(&text).ok_or(LocalError::EmptyTranscript)?;
        Ok(TranscriptionResult {
            text,
            backend: "whisper_dll".into(),
        })
    }
}

fn whisper_vad_model_path(command_path: &Option<String>) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("MUNDUS_WHISPER_CPP_VAD_MODEL") {
        let path = PathBuf::from(path.trim());
        if path.is_file() {
            return Some(path);
        }
    }
    command_path
        .as_deref()
        .map(Path::new)
        .and_then(Path::parent)
        .map(|dir| dir.join("ggml-silero-v6.2.0.bin"))
        .filter(|path| path.is_file())
}

unsafe extern "C" fn whisper_abort_callback(data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    unsafe { &*(data.cast::<AtomicBool>()) }.load(Ordering::SeqCst)
}

pub(crate) fn whisper_dll_available(model: &LocalSttModelSpec) -> bool {
    model
        .command_path
        .as_deref()
        .and_then(|path| whisper_dll_path(Path::new(path)).ok())
        .is_some_and(|path| path.is_file())
}

fn whisper_dll_path(command_path: &Path) -> Result<PathBuf, LocalError> {
    let path = command_path.with_file_name("whisper.dll");
    if path.is_file() {
        Ok(path)
    } else {
        Err(LocalError::SidecarUnavailable(format!(
            "whisper.dll not found at {}",
            path.display()
        )))
    }
}

fn load_ggml_backends(dll_dir: &Path) -> Result<Option<Library>, LocalError> {
    let ggml_path = dll_dir.join("ggml.dll");
    if !ggml_path.is_file() {
        return Ok(None);
    }
    prepend_process_path(dll_dir);
    let library = unsafe { load_library(&ggml_path) }.map_err(|e| {
        LocalError::SidecarUnavailable(format!(
            "failed to load ggml.dll at {}: {e}",
            ggml_path.display()
        ))
    })?;
    let dll_dir_c = cstring_path(dll_dir)?;
    unsafe {
        let load_all = library
            .get::<GgmlBackendLoadAllFromPath>(b"ggml_backend_load_all_from_path\0")
            .map_err(|e| {
                LocalError::SidecarUnavailable(format!(
                    "ggml.dll missing symbol ggml_backend_load_all_from_path: {e}"
                ))
            })?;
        load_all(dll_dir_c.as_ptr());
    }
    Ok(Some(library))
}

fn prepend_process_path(dir: &Path) {
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&current_path));
    if let Ok(joined) = std::env::join_paths(paths) {
        std::env::set_var("PATH", joined);
    }
}

#[cfg(windows)]
unsafe fn load_library(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::windows::{
        Library as WindowsLibrary, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    };

    unsafe {
        WindowsLibrary::load_with_flags(
            path,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    }
    .map(Into::into)
}

#[cfg(not(windows))]
unsafe fn load_library(path: &Path) -> Result<Library, libloading::Error> {
    unsafe { Library::new(path) }
}

unsafe fn load_api(library: &Library) -> Result<WhisperApi, LocalError> {
    unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, LocalError> {
        unsafe { library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|e| {
                LocalError::SidecarUnavailable(format!(
                    "whisper.dll missing symbol {}: {e}",
                    String::from_utf8_lossy(name).trim_end_matches('\0')
                ))
            })
    }

    Ok(WhisperApi {
        context_default_params: symbol(library, b"whisper_context_default_params\0")?,
        full_default_params: symbol(library, b"whisper_full_default_params\0")?,
        init_from_file_with_params: symbol(library, b"whisper_init_from_file_with_params\0")?,
        free: symbol(library, b"whisper_free\0")?,
        full: symbol(library, b"whisper_full\0")?,
        full_n_segments: symbol(library, b"whisper_full_n_segments\0")?,
        full_get_segment_text: symbol(library, b"whisper_full_get_segment_text\0")?,
    })
}

fn cstring_path(path: &Path) -> Result<CString, LocalError> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|e| {
        LocalError::CommandFailed(format!(
            "path contains interior nul {}: {e}",
            path.display()
        ))
    })
}

fn wav_pcm16_to_f32_16k_mono(wav: &[u8]) -> Result<Vec<f32>, LocalError> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(LocalError::CommandFailed("unsupported WAV header".into()));
    }
    let channels = u16::from_le_bytes([wav[22], wav[23]]);
    let sample_rate = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
    let bits = u16::from_le_bytes([wav[34], wav[35]]);
    if sample_rate != 16_000 || bits != 16 || channels == 0 {
        return Err(LocalError::CommandFailed(format!(
            "whisper.dll path expects 16kHz 16-bit PCM WAV, got {sample_rate}Hz {bits}-bit \
                 {channels}ch"
        )));
    }

    let mut offset = 12usize;
    while offset + 8 <= wav.len() {
        let chunk = &wav[offset..offset + 4];
        let len = u32::from_le_bytes([
            wav[offset + 4],
            wav[offset + 5],
            wav[offset + 6],
            wav[offset + 7],
        ]) as usize;
        let data_start = offset + 8;
        let data_end = data_start.saturating_add(len).min(wav.len());
        if chunk == b"data" {
            let frame_bytes = channels as usize * 2;
            let mut samples = Vec::with_capacity((data_end - data_start) / frame_bytes.max(1));
            for frame in wav[data_start..data_end].chunks_exact(frame_bytes) {
                let mut acc = 0.0f32;
                for channel in 0..channels as usize {
                    let i = channel * 2;
                    let sample = i16::from_le_bytes([frame[i], frame[i + 1]]) as f32 / 32768.0;
                    acc += sample;
                }
                samples.push(acc / channels as f32);
            }
            return Ok(samples);
        }
        offset = data_start + len + (len % 2);
    }

    Err(LocalError::CommandFailed("WAV data chunk not found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vad_model_is_discovered_next_to_whisper_command() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let command = tmp.path().join("whisper-cli.exe");
        let vad = tmp.path().join("ggml-silero-v6.2.0.bin");
        fs::write(&command, b"exe").expect("command");
        fs::write(&vad, b"vad").expect("vad");

        assert_eq!(
            whisper_vad_model_path(&Some(command.to_string_lossy().into_owned())),
            Some(vad)
        );
    }
}
