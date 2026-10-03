//! The static catalogue of downloadable local dictation models.

use super::LocalModelsError;

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: Option<&'static str>,
    pub size_mb: u64,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub recommended: bool,
    pub transcription_supported: bool,
    pub directory: bool,
}

pub const MODEL_CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny-q5_1",
        name: "Whisper Tiny",
        description: "Самая быстрая проверочная модель. Подходит для smoke-теста, качество ниже.",
        filename: "ggml-tiny-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny-q5_1.bin",
        sha256: None,
        size_mb: 31,
        accuracy_score: 0.35,
        speed_score: 0.98,
        recommended: false,
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "small",
        name: "Whisper Small",
        description: "Быстрая и достаточно точная модель для повседневной диктовки.",
        filename: "ggml-small.bin",
        url: "https://blob.handy.computer/ggml-small.bin",
        sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
        size_mb: 465,
        accuracy_score: 0.60,
        speed_score: 0.85,
        recommended: true,
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "medium",
        name: "Whisper Medium",
        description: "Выше качество, заметно тяжелее для CPU.",
        filename: "whisper-medium-q4_1.bin",
        url: "https://blob.handy.computer/whisper-medium-q4_1.bin",
        sha256: Some("79283fc1f9fe12ca3248543fbd54b73292164d8df5a16e095e2bceeaaabddf57"),
        size_mb: 469,
        accuracy_score: 0.75,
        speed_score: 0.60,
        recommended: false,
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "turbo",
        name: "Whisper Large v3 Turbo",
        description: "Лучший баланс качества и скорости для сильной машины.",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://blob.handy.computer/ggml-large-v3-turbo.bin",
        sha256: Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"),
        size_mb: 1549,
        accuracy_score: 0.80,
        speed_score: 0.40,
        recommended: false,
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "large",
        name: "Whisper Large v3 q5",
        description: "Самое высокое качество из локального списка, но медленнее.",
        filename: "ggml-large-v3-q5_0.bin",
        url: "https://blob.handy.computer/ggml-large-v3-q5_0.bin",
        sha256: Some("d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1"),
        size_mb: 1031,
        accuracy_score: 0.85,
        speed_score: 0.30,
        recommended: false,
        transcription_supported: true,
        directory: false,
    },
    ModelSpec {
        id: "parakeet-tdt-0.6b-v3",
        name: "Parakeet V3",
        description: "Быстрая int8-сборка NVIDIA Parakeet v3 из Handy.",
        filename: "parakeet-tdt-0.6b-v3-int8",
        url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz",
        sha256: Some("43d37191602727524a7d8c6da0eef11c4ba24320f5b4730f1a2497befc2efa77"),
        size_mb: 456,
        accuracy_score: 0.80,
        speed_score: 0.85,
        recommended: false,
        transcription_supported: true,
        directory: true,
    },
];

pub(super) fn model_spec(model_id: &str) -> Result<&'static ModelSpec, LocalModelsError> {
    MODEL_CATALOG
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| LocalModelsError::ModelNotFound(model_id.to_owned()))
}

pub fn model_supports_transcription(model_id: &str) -> Result<bool, LocalModelsError> {
    Ok(model_spec(model_id)?.transcription_supported)
}
