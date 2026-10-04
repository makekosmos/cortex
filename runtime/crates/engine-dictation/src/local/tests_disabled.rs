// KOS-337: контракт выключенного backend'а. Компилируется только без
// `local-dictation`; с фичей живёт `local/tests.rs`.
mod tests {
    use super::*;

    #[tokio::test]
    async fn transcribe_reports_not_built_without_local_dictation_feature() {
        let err = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
            idle_unload_ms: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::NotBuiltWithLocalDictation));
        assert!(err.to_string().contains("local-dictation"));
    }

    #[tokio::test]
    async fn transcribe_parakeet_reports_not_built_without_local_dictation_feature() {
        let err = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: PARAKEET_LOCAL_ENGINE,
            model_id: Some("parakeet-tdt-0.6b-v3"),
            model_path: Some("Z:/missing/parakeet"),
            command_path: None,
            idle_unload_ms: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::NotBuiltWithLocalDictation));
    }

    #[tokio::test]
    async fn preload_reports_not_built_without_local_dictation_feature() {
        let err = preload_server(
            DEFAULT_LOCAL_ENGINE,
            Some("Z:/missing/model.bin"),
            Some("Z:/missing/whisper-cli.exe"),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::NotBuiltWithLocalDictation));
    }

    #[tokio::test]
    async fn direct_whisper_backend_reports_not_built_without_feature() {
        let err = transcribe_with_whisper_backend(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
            idle_unload_ms: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::NotBuiltWithLocalDictation));
    }
}
