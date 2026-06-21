use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttModelSpec {
    pub engine: String,
    pub model_id: Option<String>,
    pub model_path: Option<String>,
    pub command_path: Option<String>,
    #[serde(default)]
    pub accelerator: LocalSttAccelerator,
    #[serde(default)]
    pub profile: LocalSttProfile,
    #[serde(default)]
    pub idle_unload_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LocalSttAccelerator {
    #[default]
    Auto,
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LocalSttProfile {
    #[default]
    Fast,
    Accurate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttRequestEnvelope {
    pub request_id: u64,
    #[serde(flatten)]
    pub request: LocalSttRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LocalSttRequest {
    Status,
    LoadModel {
        model: LocalSttModelSpec,
    },
    Preload {
        model: LocalSttModelSpec,
    },
    Transcribe {
        model: LocalSttModelSpec,
        wav_base64: String,
        language: String,
        prompt: String,
    },
    Cancel {
        target_request_id: Option<u64>,
    },
    Unload,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttResponseEnvelope {
    pub request_id: u64,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<LocalSttResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LocalSttResponse {
    Status(LocalSttStatus),
    Ack(LocalSttAck),
    Transcription(LocalSttTranscription),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttStatus {
    pub warm: bool,
    pub loaded_model: Option<LocalSttModelSpec>,
    pub backend: Option<String>,
    pub accelerator: LocalSttAccelerator,
    pub device: Option<String>,
    pub profile: LocalSttProfile,
    pub idle_unload_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttAck {
    pub accepted: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalSttTranscription {
    pub text: String,
    pub backend: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_preserves_operation_payload() {
        let envelope = LocalSttRequestEnvelope {
            request_id: 42,
            request: LocalSttRequest::Transcribe {
                model: LocalSttModelSpec {
                    engine: "whisper.cpp".into(),
                    model_id: Some("small".into()),
                    model_path: Some("C:/models/small.bin".into()),
                    command_path: Some("C:/tools/whisper-cli.exe".into()),
                    accelerator: LocalSttAccelerator::Gpu,
                    profile: LocalSttProfile::Accurate,
                    idle_unload_after_ms: Some(300_000),
                },
                wav_base64: "d2F2".into(),
                language: "ru".into(),
                prompt: "term".into(),
            },
        };

        let json = serde_json::to_string(&envelope).expect("serialize request");
        let decoded: LocalSttRequestEnvelope =
            serde_json::from_str(&json).expect("deserialize request");

        assert_eq!(decoded, envelope);
    }

    #[test]
    fn response_roundtrip_preserves_status_payload() {
        let envelope = LocalSttResponseEnvelope {
            request_id: 7,
            ok: true,
            response: Some(LocalSttResponse::Status(LocalSttStatus {
                warm: true,
                loaded_model: Some(LocalSttModelSpec {
                    engine: "whisper.cpp".into(),
                    model_id: Some("turbo".into()),
                    model_path: Some("C:/models/turbo.bin".into()),
                    command_path: Some("C:/tools/whisper-cli.exe".into()),
                    accelerator: LocalSttAccelerator::Gpu,
                    profile: LocalSttProfile::Fast,
                    idle_unload_after_ms: Some(300_000),
                }),
                backend: Some("whisper_server".into()),
                accelerator: LocalSttAccelerator::Gpu,
                device: Some("NVIDIA GeForce RTX 5070".into()),
                profile: LocalSttProfile::Fast,
                idle_unload_after_ms: Some(300_000),
            })),
            error: None,
        };

        let json = serde_json::to_string(&envelope).expect("serialize response");
        let decoded: LocalSttResponseEnvelope =
            serde_json::from_str(&json).expect("deserialize response");

        assert_eq!(decoded, envelope);
    }
}
