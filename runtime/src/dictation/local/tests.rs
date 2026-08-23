mod tests {
    use super::*;

    #[test]
    fn sidecar_candidates_include_current_platform_names() {
        let current_exe = if cfg!(windows) {
            Path::new(r"C:\Kosmos\resources\Kosmos Runtime.exe")
        } else {
            Path::new("/opt/kosmos/kosmos-runtime")
        };
        let candidates = local_stt_sidecar_candidate_paths(current_exe);
        let rendered = candidates
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        if cfg!(windows) {
            assert!(rendered
                .iter()
                .any(|path| path.ends_with("kosmos-local-stt.exe")));
            assert!(rendered
                .iter()
                .any(|path| path.ends_with("Kosmos Local STT.exe")));
        } else {
            assert!(rendered
                .iter()
                .any(|path| path.ends_with("kosmos-local-stt")));
        }
    }

    fn install_test_sidecar_mock(mock: Option<TestSidecarMock>) {
        let guard = match test_sidecar_mock_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut guard = guard;
        *guard = mock;
    }

    fn recorded_test_sidecar_ops() -> Vec<String> {
        let guard = match test_sidecar_mock_state().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .as_ref()
            .map(|mock| mock.seen_ops.clone())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn local_override_is_available_in_unit_tests() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_MODE");
        env::remove_var("KOSMOS_HEADLESS");
        env::set_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT", "local transcript");
        assert_eq!(
            test_override_transcript().as_deref(),
            Some("local transcript")
        );

        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
    }

    #[tokio::test]
    async fn whisper_cpp_default_idle_unloads_after_timeout() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");

        assert_eq!(
            local_stt_idle_unload_after_ms_for_engine(DEFAULT_LOCAL_ENGINE),
            Some(5 * 60 * 1000)
        );

        env::set_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS", "1234");
        assert_eq!(
            local_stt_idle_unload_after_ms_for_engine(DEFAULT_LOCAL_ENGINE),
            Some(1234)
        );
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");
    }

    #[tokio::test]
    async fn resolve_direct_idle_unload_uses_config_then_env_override() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");

        // Без env var — возвращает config value.
        assert_eq!(resolve_direct_idle_unload_ms(Some(300_000)), Some(300_000));
        assert_eq!(resolve_direct_idle_unload_ms(None), None);
        assert_eq!(resolve_direct_idle_unload_ms(Some(60_000)), Some(60_000));

        // Env var переопределяет config.
        env::set_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS", "9999");
        assert_eq!(resolve_direct_idle_unload_ms(Some(300_000)), Some(9999));
        assert_eq!(resolve_direct_idle_unload_ms(None), Some(9999));
        env::remove_var("KOSMOS_LOCAL_STT_IDLE_UNLOAD_MS");
    }

    #[tokio::test]
    async fn local_transcribe_errors_without_model_path() {
        let err = transcribe_with_whisper_backend(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("whisper-base"),
            model_path: None,
            command_path: Some("C:/tools/whisper-cli.exe"),
            idle_unload_ms: None,
        })
        .await
        .unwrap_err();
        assert!(matches!(err, LocalError::MissingModelPath));
    }

    fn fake_wav(sample_rate: u32, channels: u16, bits: u16, data_len: usize) -> Vec<u8> {
        let mut wav = vec![0u8; 44 + data_len];
        wav[22..24].copy_from_slice(&channels.to_le_bytes());
        wav[24..28].copy_from_slice(&sample_rate.to_le_bytes());
        wav[34..36].copy_from_slice(&bits.to_le_bytes());
        wav
    }

    #[test]
    fn wav_duration_secs_reads_header() {
        // 16kHz mono 16-bit, 2 секунды = 16000 * 2 * 2 = 64000 байт данных.
        let wav = fake_wav(16000, 1, 16, 64000);
        assert!((wav_duration_secs(&wav) - 2.0).abs() < 1e-6);
        // Слишком короткий буфер не паникует.
        assert_eq!(wav_duration_secs(b"short"), 0.0);
    }

    #[test]
    fn local_inference_timeout_scales_with_duration() {
        // Короткое аудио — не ниже минимума 60с.
        let short = fake_wav(16000, 1, 16, 16000); // 0.5с
        assert_eq!(local_inference_timeout(&short), Duration::from_secs(60));
        // Минута аудио → база 30 + 60*8 = 510с (> старых 60с, которые баговали).
        let minute = fake_wav(16000, 1, 16, 16000 * 2 * 60);
        assert_eq!(local_inference_timeout(&minute), Duration::from_secs(510));
    }

    #[tokio::test]
    async fn server_ready_timeout_is_configurable_and_clamped() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(30_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "25000");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(25_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "1");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(1_000)
        );

        env::set_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS", "999999");
        assert_eq!(
            local_stt_server_ready_timeout(),
            Duration::from_millis(300_000)
        );
        env::remove_var("KOSMOS_LOCAL_STT_SERVER_READY_TIMEOUT_MS");
    }

    #[test]
    fn strip_whisper_timestamps_removes_segment_prefixes() {
        assert_eq!(
            strip_whisper_timestamps("[00:00:00.000 --> 00:00:01.280]   Алло, алло, привет.\n"),
            "Алло, алло, привет."
        );
        assert_eq!(
            strip_whisper_timestamps(
                "[00:00:00.000 --> 00:00:01.280] first chunk\n[00:00:30.000 --> 00:00:31.000] second chunk"
            ),
            "first chunk second chunk"
        );
    }

    #[test]
    fn clean_whisper_transcript_collapses_exact_double_result() {
        let text = "Now I use dictation through Whisper.";
        assert_eq!(
            clean_whisper_transcript(&format!("{text}{text}")).as_deref(),
            Some(text)
        );
        assert_eq!(
            clean_whisper_transcript(&format!("{text} {text}")).as_deref(),
            Some(text)
        );
    }

    #[test]
    fn read_transcript_prefers_clean_txt_over_timestamp_stdout() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let out_base = tmp.path().join("dictation");
        fs::write(out_base.with_extension("txt"), "Алло, алло, привет.\n").expect("txt");

        let text = read_transcript(b"[00:00:00.000 --> 00:00:01.280]   noisy stdout", &out_base)
            .expect("transcript");

        assert_eq!(text, "Алло, алло, привет.");
    }

    #[test]
    fn whisper_server_executable_is_the_release_sibling() {
        let command = Path::new("C:/sample/local-stt/Release/whisper-cli.exe");
        assert_eq!(
            whisper_server_executable(command),
            PathBuf::from("C:/sample/local-stt/Release/whisper-server.exe")
        );
    }

    #[test]
    fn parse_server_text_prefers_json_text_field() {
        let text = parse_server_text(
            r#"{"text":"[00:00:00.000 --> 00:00:01.280] привет"}"#,
            Some("application/json"),
        )
        .expect("json transcript");
        assert_eq!(text, "привет");
    }

    #[test]
    fn parse_server_text_accepts_plain_text() {
        let text = parse_server_text(
            "[00:00:00.000 --> 00:00:01.280]   добрый день",
            Some("text/plain"),
        )
        .expect("plain transcript");
        assert_eq!(text, "добрый день");
    }

    #[test]
    fn parse_server_text_drops_no_speech_verbose_segments() {
        let err = parse_server_text(
            r#"{"text":"Продолжение следует","segments":[{"text":"Продолжение следует","no_speech_prob":0.95,"avg_logprob":-0.4}]}"#,
            Some("application/json"),
        )
        .unwrap_err();
        assert!(matches!(err, LocalError::EmptyTranscript));
    }

    #[test]
    fn parse_server_text_drops_known_subtitle_hallucination() {
        let err = parse_server_text("Субтитры сделал DimaTorzok", Some("text/plain")).unwrap_err();
        assert!(matches!(err, LocalError::EmptyTranscript));
    }

    #[test]
    fn server_request_form_includes_expected_fields() {
        let req = OwnedLocalRequest {
            wav_bytes: b"wav".to_vec(),
            language: "ru".into(),
            prompt: "term".into(),
            engine: DEFAULT_LOCAL_ENGINE.into(),
            model_id: None,
            model_path: Some("C:/models/ggml-base.bin".into()),
            command_path: Some("C:/tools/whisper-cli.exe".into()),
            accelerator: LocalSttAccelerator::Auto,
            profile: LocalSttProfile::Fast,
            idle_unload_ms: None,
        };

        let form = server_request_form(&req).expect("form");
        let debug = format!("{form:?}");
        assert!(debug.contains("file"));
        assert!(debug.contains("response_format"));
        assert!(debug.contains("language"));
        assert!(debug.contains("prompt"));
    }

    #[tokio::test]
    async fn preload_server_returns_false_without_server_binary() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let model = tmp.path().join("model.bin");
        let command = tmp.path().join("whisper-cli.exe");
        fs::write(&model, b"model").expect("model");
        fs::write(&command, b"exe").expect("command");

        let warmed =
            preload_with_whisper_backend(DEFAULT_LOCAL_ENGINE, model.to_str(), command.to_str())
                .await
                .expect("preload");

        assert!(!warmed);
    }

    #[tokio::test]
    async fn preload_server_rejects_unsupported_engine() {
        let err = preload_with_whisper_backend("other", None, None)
            .await
            .unwrap_err();

        assert!(matches!(err, LocalError::UnsupportedEngine { .. }));
    }

    #[tokio::test]
    async fn preload_server_prefers_mocked_sidecar() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: None,
            fail_error: None,
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let warmed = preload_server(
            DEFAULT_LOCAL_ENGINE,
            Some("Z:/missing/model.bin"),
            Some("Z:/missing/whisper-cli.exe"),
        )
        .await
        .expect("preload through sidecar");

        assert!(warmed);
        assert_eq!(recorded_test_sidecar_ops(), vec!["preload"]);
        install_test_sidecar_mock(None);
        clear_test_sidecar_pool_for_host().await;
    }

    #[test]
    fn whisper_profile_and_accelerator_args_map_to_backend_flags() {
        let mut fast = Command::new("whisper-cli");
        apply_whisper_quality_args_blocking(&mut fast, &LocalSttProfile::Fast);
        let fast_args = fast
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(fast_args, vec!["-bo", "1", "-bs", "1"]);

        let mut accurate = Command::new("whisper-cli");
        apply_whisper_quality_args_blocking(&mut accurate, &LocalSttProfile::Accurate);
        let accurate_args = accurate
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(accurate_args, vec!["-bo", "5", "-bs", "5"]);

        let mut cpu = Command::new("whisper-cli");
        apply_whisper_accelerator_args_blocking(&mut cpu, &LocalSttAccelerator::Cpu);
        let cpu_args = cpu
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(cpu_args, vec!["-ng"]);

        let mut gpu = Command::new("whisper-cli");
        apply_whisper_accelerator_args_blocking(&mut gpu, &LocalSttAccelerator::Gpu);
        assert!(gpu.get_args().next().is_none());
    }

    #[test]
    fn whisper_vad_args_are_added_when_vad_model_exists_next_to_command() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let command_path = tmp.path().join("whisper-cli.exe");
        let vad_path = tmp.path().join("ggml-silero-v6.2.0.bin");
        fs::write(&command_path, b"exe").expect("command");
        fs::write(&vad_path, b"vad").expect("vad");

        let mut command = Command::new("whisper-cli");
        apply_whisper_vad_args_blocking(&mut command, &command_path);

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            vec![
                "--vad".to_owned(),
                "-vm".to_owned(),
                vad_path.to_string_lossy().into_owned()
            ]
        );
    }

    #[tokio::test]
    async fn transcribe_prefers_mocked_sidecar_over_direct_whisper_binaries() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: Some("sidecar transcript".into()),
            fail_error: None,
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
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
        .expect("sidecar transcript");

        assert_eq!(result.text, "sidecar transcript");
        assert_eq!(recorded_test_sidecar_ops(), vec!["transcribe"]);
        install_test_sidecar_mock(None);
    }

    #[tokio::test]
    async fn transcribe_cleans_sidecar_transcript_before_returning() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        let text = "This is a long dictation transcript that should only appear once.";
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: Some(format!("{text}\n{text}")),
            fail_error: None,
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
            wav_bytes: b"wav",
            language: "ru",
            prompt: "",
            engine: DEFAULT_LOCAL_ENGINE,
            model_id: Some("small"),
            model_path: Some("Z:/missing/model.bin"),
            command_path: Some("Z:/missing/whisper-cli.exe"),
            idle_unload_ms: None,
        })
        .await;

        install_test_sidecar_mock(None);
        assert_eq!(
            result.expect("sidecar transcript").text,
            text,
            "Regression: 2026-07-03. Sidecar raw text must still pass shared cleanup."
        );
    }

    #[tokio::test]
    async fn transcribe_retries_once_after_sidecar_unavailable() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: Some("recovered transcript".into()),
            fail_error: None,
            sidecar_unavailable_remaining: 1,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
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
        .expect("recovered transcript");

        assert_eq!(result.text, "recovered transcript");
        assert_eq!(
            recorded_test_sidecar_ops(),
            vec!["transcribe", "transcribe"]
        );
        install_test_sidecar_mock(None);
    }

    #[tokio::test]
    async fn test_override_transcript_bypasses_sidecar_requests() {
        let _guard = TEST_SIDECAR_TEST_LOCK.lock().await;
        env::remove_var("KOSMOS_TEST_DICTATION_TRANSCRIPT");
        env::set_var(
            "KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT",
            "override transcript",
        );
        install_test_sidecar_mock(Some(TestSidecarMock {
            transcript: None,
            fail_error: Some("sidecar should not be called".into()),
            sidecar_unavailable_remaining: 0,
            seen_ops: Vec::new(),
        }));

        let result = transcribe(LocalRequest {
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
        .expect("override transcript");

        assert_eq!(result.text, "override transcript");
        assert!(recorded_test_sidecar_ops().is_empty());

        install_test_sidecar_mock(None);
        env::remove_var("KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT");
    }
}
