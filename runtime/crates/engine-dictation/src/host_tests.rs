    use super::*;

    // ---- validate_config_patch ----

    #[test]
    fn validate_empty_patch_is_ok() {
        assert!(validate_config_patch(&json!({})).is_ok());
    }

    #[test]
    fn validate_unrelated_fields_ok() {
        let patch = json!({
            "hotkey": "Ctrl+Shift+;",
            "language": "ru",
            "triggerMode": "toggle",
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_system_profile_ok() {
        let patch = json!({ "networkProfile": { "kind": "system" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_cloudflare_doh_ok() {
        let patch = json!({ "networkProfile": { "kind": "cloudflare_doh" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_google_doh_ok() {
        let patch = json!({ "networkProfile": { "kind": "google_doh" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_unknown_profile_kind_errors() {
        let patch = json!({ "networkProfile": { "kind": "tor_hidden" } });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("tor_hidden"));
    }

    #[test]
    fn validate_custom_doh_with_valid_url_ok() {
        let patch = json!({
            "networkProfile": {
                "kind": "custom_doh",
                "url": "https://comss.dns.controld.com/dns-query"
            }
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_with_literal_ip_ok() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "https://1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_empty_url_ok_transient() {
        // Транзитное состояние: юзер только что переключил radio на custom_doh,
        // URL ещё не введён. Валидация пропускается; build_client при
        // транскрибе вернёт CustomDohInvalid если URL так и не появится.
        let patch = json!({ "networkProfile": { "kind": "custom_doh", "url": "" } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_whitespace_url_ok_transient() {
        let patch = json!({ "networkProfile": { "kind": "custom_doh", "url": "   " } });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_custom_doh_without_https_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "comss.dns.controld.com/dns-query" }
        });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("Custom DoH"));
        assert!(err.contains("https://"));
    }

    #[test]
    fn validate_custom_doh_http_scheme_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "http://1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_err());
    }

    #[test]
    fn validate_custom_doh_userinfo_errors() {
        let patch = json!({
            "networkProfile": { "kind": "custom_doh", "url": "https://user:pass@1.1.1.1/dns-query" }
        });
        assert!(validate_config_patch(&patch).is_err());
    }

    #[test]
    fn validate_custom_doh_missing_url_field_ok_transient() {
        let patch = json!({ "networkProfile": { "kind": "custom_doh" } });
        // url field отсутствует — тоже транзитно ok (фронт мог не передать).
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_valid_ok() {
        let patch = json!({ "httpProxy": "http://127.0.0.1:8080" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_socks5_ok() {
        let patch = json!({ "httpProxy": "socks5://127.0.0.1:1080" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_invalid_errors() {
        let patch = json!({ "httpProxy": "not a url" });
        let err = validate_config_patch(&patch).unwrap_err();
        assert!(err.contains("proxy"));
    }

    #[test]
    fn validate_http_proxy_null_ok() {
        let patch = json!({ "httpProxy": null });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_empty_string_ok() {
        let patch = json!({ "httpProxy": "" });
        assert!(validate_config_patch(&patch).is_ok());
    }

    #[test]
    fn validate_http_proxy_whitespace_ok() {
        let patch = json!({ "httpProxy": "   " });
        assert!(validate_config_patch(&patch).is_ok());
    }

    // ---- probe_connectivity ----

    #[tokio::test]
    async fn probe_reports_all_stages_on_success() {
        // httpmock — настоящий HTTP сервер, robust для probe testing.
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method("HEAD").path("/");
                then.status(200);
            })
            .await;
        let port = server.port();
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            port,
            &format!("http://127.0.0.1:{port}/"),
        )
        .await;
        assert_eq!(report["ok"], true, "report: {report}");
        let stages = report["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 4, "должно быть 4 стадии");
        let names: Vec<_> = stages.iter().map(|s| s["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec!["client_build", "dns_resolve", "tcp_connect", "http_head"]
        );
        for s in stages {
            assert_eq!(s["ok"], true, "stage failed: {s}");
        }
        let http_stage = stages.iter().find(|s| s["name"] == "http_head").unwrap();
        assert_eq!(http_stage["status"], 200);
        assert!(report["firstFailure"].is_null());
        assert!(report["totalMs"].as_u64().unwrap() < 60_000);
    }

    #[tokio::test]
    async fn probe_first_failure_on_tcp_refused() {
        // Несуществующий локальный порт → resolve OK, TCP connect fail.
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            1, // reserved, не принимает коннекты
            "http://127.0.0.1:1/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "tcp_connect");
        let stages = report["stages"].as_array().unwrap();
        // client_build + dns_resolve OK, tcp_connect FAIL, http_head не выполнялся
        assert_eq!(stages[0]["name"], "client_build");
        assert_eq!(stages[0]["ok"], true);
        assert_eq!(stages[1]["name"], "dns_resolve");
        assert_eq!(stages[1]["ok"], true);
        assert_eq!(stages[2]["name"], "tcp_connect");
        assert_eq!(stages[2]["ok"], false);
        assert!(
            stages.len() == 3,
            "http_head должен быть skipped: {stages:?}"
        );
    }

    #[tokio::test]
    async fn probe_first_failure_on_invalid_proxy() {
        // Невалидный proxy → client_build fail; остальные стадии skipped.
        let report = probe_connectivity(
            &NetworkProfile::System,
            Some("not a url"),
            "127.0.0.1",
            443,
            "http://127.0.0.1/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "client_build");
        let stages = report["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 1, "только client_build, остальное skipped");
        assert_eq!(stages[0]["ok"], false);
    }

    #[tokio::test]
    async fn probe_first_failure_on_dns_resolve_unknown_host() {
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "definitely-not-a-real-host-12345.invalid",
            443,
            "https://definitely-not-a-real-host-12345.invalid/",
        )
        .await;
        assert_eq!(report["ok"], false);
        assert_eq!(report["firstFailure"], "dns_resolve");
        let stages = report["stages"].as_array().unwrap();
        // client_build OK, dns_resolve FAIL, tcp/http skipped
        assert!(stages
            .iter()
            .any(|s| s["name"] == "dns_resolve" && s["ok"] == false));
    }

    #[tokio::test]
    async fn probe_includes_resolved_ip_on_success() {
        let report = probe_connectivity(
            &NetworkProfile::System,
            None,
            "127.0.0.1",
            65535, // даже не открываем TCP, нам нужна только DNS стадия
            "http://127.0.0.1:65535/",
        )
        .await;
        let stages = report["stages"].as_array().unwrap();
        let dns_stage = stages.iter().find(|s| s["name"] == "dns_resolve").unwrap();
        assert_eq!(dns_stage["ok"], true);
        assert_eq!(dns_stage["ip"], "127.0.0.1");
    }

    // ---- process_pending integration ----

    fn test_cfg() -> DictationConfig {
        DictationConfig {
            hotkey: "Ctrl+Shift+;".into(),
            trigger_mode: TriggerMode::Toggle,
            language: "ru".into(),
            inject_mode: InjectMode::ClipboardOnly, // не трогаем реальный clipboard
            pill_style: PillStyle::Large,
            network_profile: NetworkProfile::System,
            provider: "groq".into(),
            provider_enabled: true,
            duck_audio_during_recording: false,
            model: "whisper-large-v3".into(),
            local_engine: DEFAULT_LOCAL_ENGINE.into(),
            local_model_path: None,
            local_command_path: None,
            local_model: None,
            http_proxy: None,
            transcription_prompt: String::new(),
            microphone_device_id: None,
            local_idle_unload_ms: Some(300_000),
        }
    }

    fn isolated_host() -> (tempfile::TempDir, Arc<DictationHost>) {
        let data = tempfile::TempDir::new().expect("tempdir");
        let host =
            DictationHost::new_for_test(data.path().into(), groq::GROQ_ENDPOINT.into(), test_cfg());
        (data, host)
    }

    fn make_wav() -> Vec<u8> {
        // Minimal valid WAV header + 1 sample silence. Достаточно для теста
        // что pending::enqueue/read_wav круглим без потерь.
        let mut wav = Vec::with_capacity(44);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&36u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&16000u32.to_le_bytes()); // sample rate
        wav.extend_from_slice(&32000u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes());
        wav
    }

    fn add_local_whisper_cpp_files(data_dir: &std::path::Path, cfg: &mut DictationConfig) {
        let model_path = data_dir.join("test-model.bin");
        let command_path = data_dir.join("whisper-cli.exe");
        std::fs::write(&model_path, b"model").expect("model file");
        std::fs::write(&command_path, b"command").expect("command file");
        cfg.local_model_path = Some(model_path.to_string_lossy().to_string());
        cfg.local_command_path = Some(command_path.to_string_lossy().to_string());
    }

    struct FailingInjector;

    impl crate::inject::Injector for FailingInjector {
        fn inject(
            &self,
            _text: &str,
            _mode: InjectMode,
            _prev_hwnd: Option<isize>,
        ) -> Result<crate::inject::DeliveryResult, InjectError> {
            Err(InjectError::SendInput {
                injected: 0,
                expected: 4,
            })
        }
    }

    #[tokio::test]
    async fn process_pending_success_transitions_to_idle_and_drops_item() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200).body(concat!(
                    r#"{"text":"привет","segments":[{"text":"привет","no_speech_prob":0.05,"#,
                    r#""avg_logprob":-0.3}]}"#
                ));
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());

        // pre-enqueue
        let wav = make_wav();
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &wav,
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Симулируем активную сессию — иначе process_one_attempt не трогает state.
        {
            let mut s = host.state.lock().await;
            s.active_uuid = Some(uuid.clone());
            s.name = DictationStateName::Transcribing;
        }

        let outcome =
            process_one_attempt(&host, &uuid, "fake-key", 1.0, AttemptDelivery::Active).await;
        assert!(matches!(
            outcome,
            AttemptOutcome::Success {
                injected: false,
                delivery: super::super::inject::Delivery::ClipboardOnly,
                ..
            }
        ));

        // Доставленная попытка остаётся историей, но покидает очередь.
        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status.as_deref(), Some("delivered"));
        assert!(super::super::pending::list_unresolved(&host.data_dir)
            .unwrap()
            .is_empty());
        // state → Idle
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    #[tokio::test]
    async fn background_process_success_drops_pending_without_active_session() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200).body(concat!(
                    r#"{"text":"retry transcript","segments":[{"text":"retry transcript","#,
                    r#""no_speech_prob":0.05,"avg_logprob":-0.3}]}"#
                ));
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: Some(1),
            },
        )
        .unwrap();

        let outcome =
            process_one_attempt(&host, &uuid, "fake-key", 1.0, AttemptDelivery::Background).await;

        assert!(matches!(
            outcome,
            AttemptOutcome::Success {
                injected: false,
                delivery: super::super::inject::Delivery::ClipboardOnly,
                ..
            }
        ));
        assert!(super::super::pending::list_unresolved(&host.data_dir)
            .unwrap()
            .is_empty());
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    #[tokio::test]
    async fn delivery_failure_keeps_pending_and_reports_safe_reason() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200).body(concat!(
                    r#"{"text":"delivery test","segments":[{"text":"delivery test","#,
                    r#""no_speech_prob":0.05,"avg_logprob":-0.3}]}"#
                ));
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: Some(7),
            },
        )
        .unwrap();
        {
            let mut s = host.state.lock().await;
            s.active_uuid = Some(uuid.clone());
            s.name = DictationStateName::Transcribing;
        }

        let outcome = process_one_attempt_with_injector(
            &host,
            &uuid,
            "fake-key",
            1.0,
            AttemptDelivery::Active,
            std::sync::Arc::new(FailingInjector),
        )
        .await;

        assert_eq!(
            outcome,
            AttemptOutcome::DeliveryFailed {
                reason: "paste_failed"
            }
        );
        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts, 1);
        let state = host.current_state().await;
        assert_eq!(state["state"], "error");
        assert_eq!(state["canRetry"], true);
    }

    #[tokio::test]
    async fn process_pending_401_keeps_item_and_marks_error() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(401).body(r#"{"error":"invalid key"}"#);
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());

        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Активная сессия — state.active_uuid должен matching, иначе state не обновляется.
        {
            let mut s = host.state.lock().await;
            s.active_uuid = Some(uuid.clone());
            s.name = DictationStateName::Transcribing;
        }

        let outcome =
            process_one_attempt(&host, &uuid, "fake-key", 1.0, AttemptDelivery::Active).await;
        assert_eq!(outcome, AttemptOutcome::Fatal, "401 must be Fatal");

        // Pending item остался на диске + attempts++
        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts, 1);
        // State → Error с user_msg про API key
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "error");
        assert!(
            snap["lastError"].as_str().unwrap().contains("API key"),
            "msg: {}",
            snap["lastError"]
        );
        // 401 — fatal, can_retry=false (retry без смены ключа бесполезен).
        assert_eq!(snap["canRetry"], false);
    }

    #[tokio::test]
    async fn process_one_attempt_5xx_is_retryable_keeps_pending() {
        // Одна попытка vs 503 — Retryable. Auto-retry-loop в проде дальше
        // запустит ретраи. Здесь тестим что: pending остался, attempts++,
        // outcome=Retryable, state НЕ Error (background retry в работе).
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503).body("temporarily down");
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        let outcome =
            process_one_attempt(&host, &uuid, "fake-key", 1.0, AttemptDelivery::Active).await;
        assert_eq!(outcome, AttemptOutcome::Retryable);

        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts, 1);
    }

    #[tokio::test]
    async fn auto_retry_loop_gives_up_on_persistent_retryable() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503).body("forever down");
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // 3 ретрая по 0ms = 4 попытки. Все 503 → retryable исчерпан.
        auto_retry_loop(
            host.clone(),
            uuid.clone(),
            "fake-key".into(),
            1.0,
            &[0u64, 0, 0],
        )
        .await;

        let items = super::super::pending::list(&host.data_dir).unwrap();
        assert_eq!(items.len(), 1, "pending остался");
        assert_eq!(items[0].attempts, 3, "3 attempts через auto_retry_loop");
    }

    #[tokio::test]
    async fn auto_retry_loop_stops_on_discard() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(503);
            })
            .await;
        let td = tempfile::TempDir::new().unwrap();
        let endpoint = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let host = DictationHost::new_for_test(td.path().into(), endpoint, test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: String::new(),
                inject_mode: "clipboard_only".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();

        // Удаляем pending до начала retry-loop.
        super::super::pending::drop_item(&host.data_dir, &uuid).unwrap();
        // Loop должен сразу выйти увидев что item исчез.
        auto_retry_loop(
            host.clone(),
            uuid.clone(),
            "fake-key".into(),
            1.0,
            &[0u64, 0, 0],
        )
        .await;
        // Никаких новых файлов не появилось — discard выдержан.
        assert!(super::super::pending::list_unresolved(&host.data_dir)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn op_list_pending_returns_items() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            2.5,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        let resp = op_list_pending(&host).await;
        assert!(resp.ok);
        let items = resp.data["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["language"], "ru");
        assert_eq!(items[0]["durationSec"], 2.5);
    }

    #[tokio::test]
    async fn op_discard_removes_item_and_resets_state() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        let uuid = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        // Подделаем active session.
        {
            let mut s = host.state.lock().await;
            s.name = DictationStateName::Error;
            s.active_uuid = Some(uuid.clone());
            s.last_error = Some("test".into());
        }
        let resp = op_discard(json!({ "uuid": uuid }), &host).await;
        assert!(resp.ok);
        assert!(super::super::pending::list_unresolved(&host.data_dir)
            .unwrap()
            .is_empty());
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    #[tokio::test]
    async fn op_discard_unknown_uuid_errors() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        let resp = op_discard(json!({ "uuid": "nope" }), &host).await;
        assert!(!resp.ok);
    }

    #[tokio::test]
    async fn op_discard_all_removes_items_and_resets_active_state() {
        let td = tempfile::TempDir::new().unwrap();
        let host =
            DictationHost::new_for_test(td.path().into(), "http://localhost/".into(), test_cfg());
        let first = super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            1.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        super::super::pending::enqueue(
            &host.data_dir,
            &make_wav(),
            2.0,
            super::super::pending::EnqueueOpts {
                language: "ru".into(),
                prompt: "".into(),
                inject_mode: "auto_paste".into(),
                model: "whisper-large-v3".into(),
                prev_hwnd: None,
            },
        )
        .unwrap();
        {
            let mut s = host.state.lock().await;
            s.name = DictationStateName::Error;
            s.active_uuid = Some(first);
            s.last_error = Some("test".into());
        }

        let resp = op_discard_all(&host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["discarded"], 2);
        assert!(super::super::pending::list_unresolved(&host.data_dir)
            .unwrap()
            .is_empty());
        let snap = host.current_state().await;
        assert_eq!(snap["state"], "idle");
    }

    // ---- verify_api_key ----

    #[tokio::test]
    async fn verify_api_key_empty_returns_ok_false_empty_key() {
        let r = verify_api_key_with("", &NetworkProfile::System, None, "http://unused").await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "empty_key");
    }

    #[tokio::test]
    async fn verify_api_key_whitespace_returns_empty_key() {
        let r = verify_api_key_with("  \t  ", &NetworkProfile::System, None, "http://unused").await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "empty_key");
    }

    #[tokio::test]
    async fn verify_api_key_200_returns_ok_true() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/v1/models")
                    .header("authorization", "Bearer fake-key");
                then.status(200).body(r#"{"data":[]}"#);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("fake-key", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], true);
        assert_eq!(r["status"], 200);
    }

    #[tokio::test]
    async fn verify_api_key_401_returns_invalid_key() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/v1/models");
                then.status(401).body(r#"{"error":"unauthorized"}"#);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("bad-key", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "invalid_key");
        assert_eq!(r["status"], 401);
    }

    #[tokio::test]
    async fn verify_api_key_5xx_returns_provider_error() {
        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/v1/models");
                then.status(503);
            })
            .await;
        let endpoint = format!("{}/v1/models", server.base_url());
        let r = verify_api_key_with("any", &NetworkProfile::System, None, &endpoint).await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "provider_error");
        assert_eq!(r["status"], 503);
    }

    #[tokio::test]
    async fn verify_api_key_network_fail_returns_network_reason() {
        // Закрытый порт → connect refused → reason: network.
        let r = verify_api_key_with(
            "key",
            &NetworkProfile::System,
            None,
            "http://127.0.0.1:1/v1/models",
        )
        .await;
        assert_eq!(r["ok"], false);
        assert_eq!(r["reason"], "network");
    }

    // ---- existing tests ----

    #[tokio::test]
    async fn get_state_idle_initially() {
        let (_data, host) = isolated_host();
        let resp = handle_dictation_op("get_state", Value::Null, &host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["state"], "idle", "resp: {:?}", resp.data);
    }

    #[tokio::test]
    async fn start_recording_transitions_to_recording() {
        let (_data, host) = isolated_host();
        let resp = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(resp.ok, "start_recording failed: {:?}", resp.error);
        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "recording");
    }

    // KOS-337: exercises the local backend — only built with `local-dictation`.
    #[cfg(feature = "local-dictation")]
    #[tokio::test]
    async fn start_recording_preloads_local_sidecar() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        add_local_whisper_cpp_files(td.path(), &mut cfg);
        local::install_test_sidecar_mock_for_host(None);
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );

        let resp = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(resp.ok, "start_recording failed: {:?}", resp.error);

        let mut saw_preload = false;
        for _ in 0..20 {
            if local::recorded_test_sidecar_ops_for_host()
                .iter()
                .any(|op| op == "preload")
            {
                saw_preload = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        local::clear_test_sidecar_mock_for_host();
        assert!(saw_preload, "start_recording must preload local sidecar");
    }

    #[tokio::test]
    async fn list_local_models_clears_stale_missing_local_selection() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.provider_enabled = true;
        cfg.local_model = Some("turbo".into());
        cfg.local_model_path = Some(
            td.path()
                .join("missing-model.bin")
                .to_string_lossy()
                .to_string(),
        );
        cfg.local_command_path = Some(
            td.path()
                .join("missing-whisper-cli.exe")
                .to_string_lossy()
                .to_string(),
        );
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );

        let resp = handle_dictation_op("list_local_models", Value::Null, &host).await;
        assert!(resp.ok, "list_local_models failed: {:?}", resp.error);
        let cfg = host.snapshot_config().await;
        assert!(!cfg.provider_enabled);
        assert_eq!(cfg.local_model, None);
        assert_eq!(cfg.local_model_path, None);
        assert_eq!(cfg.local_command_path, None);
    }

    #[tokio::test]
    async fn start_recording_from_non_idle_errors() {
        let (_data, host) = isolated_host();
        handle_dictation_op("start_recording", Value::Null, &host).await;
        let resp = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(!resp.ok);
        let err = resp.error.unwrap_or_default();
        assert!(err.contains("idle"), "got: {err}");
    }

    #[tokio::test]
    async fn cancel_returns_to_idle() {
        let (_data, host) = isolated_host();
        handle_dictation_op("start_recording", Value::Null, &host).await;
        {
            let mut state = host.state.lock().await;
            state.active_uuid = Some("cancel-me".into());
            state.attempts = 1;
            state.can_retry = true;
        }
        let resp = handle_dictation_op("cancel", Value::Null, &host).await;
        assert!(resp.ok);
        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "idle");
        assert!(state.data["activeUuid"].is_null());
        assert_eq!(state.data["attempts"], 0);
        assert_eq!(state.data["canRetry"], false);
    }

    #[tokio::test]
    async fn submit_audio_requires_recording_state() {
        let (_data, host) = isolated_host();
        let resp =
            handle_dictation_op("submit_audio", json!({ "audioB64": "aGVsbG8=" }), &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap_or_default().contains("recording"));
    }

    #[tokio::test]
    async fn submit_audio_provider_disabled_returns_state_error_not_ipc_error() {
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider_enabled = false;
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 1.0 }),
            &host,
        )
        .await;

        assert!(resp.ok, "provider disabled must not throw IPC error");
        assert_eq!(resp.data["state"], "error");
        assert!(
            resp.data["error"]
                .as_str()
                .unwrap_or_default()
                .contains("выключен"),
            "error: {}",
            resp.data["error"]
        );
        assert_eq!(
            op_list_pending(&host).await.data["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    // KOS-337: exercises the local backend — only built with `local-dictation`.
    #[cfg(feature = "local-dictation")]
    #[tokio::test]
    async fn submit_audio_local_provider_without_model_reports_missing_local_model() {
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.provider_enabled = false;
        cfg.local_model = None;
        cfg.local_model_path = None;
        cfg.local_command_path = None;
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 1.0 }),
            &host,
        )
        .await;

        assert!(resp.ok, "missing local model must not throw IPC error");
        assert_eq!(resp.data["state"], "error");
        assert!(
            resp.data["error"]
                .as_str()
                .unwrap_or_default()
                .contains("Локальная модель"),
            "error: {}",
            resp.data["error"]
        );
    }

    #[tokio::test]
    async fn mock_transcript_override_requires_test_mode() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        std::env::remove_var("MUNDUS_TEST_MODE");
        std::env::remove_var("MUNDUS_HEADLESS");
        std::env::set_var(
            "MUNDUS_TEST_DICTATION_TRANSCRIPT",
            "детерминированный текст",
        );

        assert_eq!(mock_dictation_transcript_override("mock", None), None);

        std::env::set_var("MUNDUS_TEST_MODE", "1");
        assert_eq!(
            mock_dictation_transcript_override("mock", None).as_deref(),
            Some("детерминированный текст")
        );
        assert_eq!(mock_dictation_transcript_override("groq", None), None);

        std::env::remove_var("MUNDUS_TEST_DICTATION_TRANSCRIPT");
        std::env::remove_var("MUNDUS_TEST_MODE");
    }

    #[test]
    fn resolve_attempt_inject_mode_forces_clipboard_only_for_mock() {
        assert_eq!(
            resolve_attempt_inject_mode("auto_paste", Some("mock transcript")),
            InjectMode::ClipboardOnly
        );
        assert_eq!(
            resolve_attempt_inject_mode("auto_paste", None),
            InjectMode::AutoPaste
        );
        assert_eq!(
            resolve_attempt_inject_mode("clipboard_only", None),
            InjectMode::ClipboardOnly
        );
    }

    #[tokio::test]
    async fn submit_audio_mock_transcript_succeeds_without_api_key_and_cleans_up() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        std::env::set_var("MUNDUS_TEST_MODE", "1");
        std::env::set_var("MUNDUS_TEST_DICTATION_TRANSCRIPT", "привет из теста");

        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "mock".into();
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        let mut rx = host.subscribe();
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 4.0 }),
            &host,
        )
        .await;
        assert!(resp.ok, "submit_audio failed: {:?}", resp.error);
        assert_eq!(resp.data["state"], "idle");

        let mut saw_transcribing = false;
        let mut saw_transcript = None;
        for _ in 0..6 {
            let evt = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
                .await
                .expect("event timeout")
                .expect("event recv");
            match evt["event"].as_str() {
                Some("dictation_state_changed") if evt["state"] == "transcribing" => {
                    saw_transcribing = true;
                }
                Some("dictation_transcript") => {
                    saw_transcript = Some(evt);
                    break;
                }
                _ => {}
            }
        }

        assert!(
            saw_transcribing,
            "submit_audio must emit transcribing state"
        );
        let transcript_evt = saw_transcript.expect("missing dictation_transcript event");
        assert_eq!(transcript_evt["text"], "привет из теста");
        assert_eq!(transcript_evt["language"], "ru");
        assert_eq!(transcript_evt["injected"], false);

        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "idle");
        assert_eq!(state.data["activeUuid"], Value::Null);

        // submit_audio обходит очередь — диагностический путь, не история.
        let pending = op_list_pending(&host).await;
        assert!(pending.ok);
        assert_eq!(pending.data["items"].as_array().unwrap().len(), 0);

        let stats = handle_dictation_op("get_stats", Value::Null, &host).await;
        assert!(stats.ok);
        assert_eq!(stats.data["totalSessions"], 1);
        assert_eq!(stats.data["totalWords"], 3);
        assert_eq!(stats.data["totalRecordSeconds"], 4);

        std::env::remove_var("MUNDUS_TEST_DICTATION_TRANSCRIPT");
        std::env::remove_var("MUNDUS_TEST_MODE");
    }

    #[tokio::test]
    async fn submit_audio_groq_transcript_succeeds_with_test_api_key_and_cleans_up() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        std::env::set_var("MUNDUS_TEST_MODE", "1");
        std::env::set_var("MUNDUS_TEST_GROQ_API_KEY", "test-groq-api-key");
        std::env::remove_var("MUNDUS_TEST_DICTATION_TRANSCRIPT");

        use httpmock::prelude::*;
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(concat!(
                        r#"{"text":"groq runtime transcript","segments":[{"text":"groq runtime "#,
                        r#"transcript","no_speech_prob":0.05,"avg_logprob":-0.2}]}"#
                    ));
            })
            .await;

        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "groq".into();
        cfg.inject_mode = InjectMode::ClipboardOnly;
        let host = DictationHost::new_for_test(
            td.path().into(),
            format!("{}/openai/v1/audio/transcriptions", server.base_url()),
            cfg,
        );
        let mut rx = host.subscribe();
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 4.0 }),
            &host,
        )
        .await;
        assert!(resp.ok, "submit_audio failed: {:?}", resp.error);
        assert_eq!(resp.data["state"], "idle");

        let mut saw_transcribing = false;
        let mut saw_transcript = None;
        let mut saw_stats = false;
        let mut saw_idle = false;
        for _ in 0..8 {
            let evt = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
                .await
                .expect("event timeout")
                .expect("event recv");
            match evt["event"].as_str() {
                Some("dictation_state_changed") if evt["state"] == "transcribing" => {
                    saw_transcribing = true;
                }
                Some("dictation_transcript") => {
                    saw_transcript = Some(evt);
                }
                Some("dictation_stats_changed") => {
                    saw_stats = true;
                }
                Some("dictation_state_changed") if evt["state"] == "idle" => {
                    saw_idle = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(
            saw_transcribing,
            "submit_audio must emit transcribing state"
        );
        let transcript_evt = saw_transcript.expect("missing dictation_transcript event");
        assert_eq!(transcript_evt["text"], "groq runtime transcript");
        assert_eq!(transcript_evt["language"], "ru");
        assert_eq!(transcript_evt["uuid"], resp.data["uuid"]);
        assert_eq!(transcript_evt["injected"], false);
        assert!(saw_stats, "submit_audio must emit dictation_stats_changed");
        assert!(saw_idle, "submit_audio must return the host to idle");

        mock.assert_async().await;

        let state = handle_dictation_op("get_state", Value::Null, &host).await;
        assert_eq!(state.data["state"], "idle");
        assert_eq!(state.data["activeUuid"], Value::Null);
        assert_eq!(state.data["config"]["injectMode"], "clipboard_only");

        let pending = op_list_pending(&host).await;
        assert!(pending.ok);
        let items = pending.data["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["status"], "delivered");

        let stats = handle_dictation_op("get_stats", Value::Null, &host).await;
        assert!(stats.ok);
        assert_eq!(stats.data["totalSessions"], 1);
        assert_eq!(stats.data["totalWords"], 3);
        assert_eq!(stats.data["totalRecordSeconds"], 4);

        std::env::remove_var("MUNDUS_TEST_GROQ_API_KEY");
        std::env::remove_var("MUNDUS_TEST_DICTATION_TRANSCRIPT");
        std::env::remove_var("MUNDUS_TEST_MODE");
    }

    // KOS-337: exercises the local backend — only built with `local-dictation`.
    #[cfg(feature = "local-dictation")]
    #[tokio::test]
    async fn submit_audio_local_transcript_succeeds_without_api_key_and_cleans_up() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        std::env::set_var("MUNDUS_TEST_MODE", "1");
        std::env::set_var(
            "MUNDUS_TEST_LOCAL_DICTATION_TRANSCRIPT",
            "локальная расшифровка",
        );

        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.local_model = Some("whisper-large-v3-turbo".into());
        add_local_whisper_cpp_files(td.path(), &mut cfg);
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        let mut rx = host.subscribe();
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 3.0 }),
            &host,
        )
        .await;
        assert!(resp.ok, "submit_audio failed: {:?}", resp.error);
        assert_eq!(resp.data["state"], "idle");

        let mut saw_transcribing = false;
        let mut saw_transcript = None;
        for _ in 0..6 {
            let evt = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
                .await
                .expect("event timeout")
                .expect("event recv");
            match evt["event"].as_str() {
                Some("dictation_state_changed") if evt["state"] == "transcribing" => {
                    saw_transcribing = true;
                }
                Some("dictation_transcript") => {
                    saw_transcript = Some(evt);
                    break;
                }
                _ => {}
            }
        }

        assert!(
            saw_transcribing,
            "submit_audio must emit transcribing state"
        );
        let transcript_evt = saw_transcript.expect("missing dictation_transcript event");
        assert_eq!(transcript_evt["text"], "локальная расшифровка");
        assert_eq!(transcript_evt["language"], "ru");
        assert_eq!(transcript_evt["injected"], false);

        let pending = op_list_pending(&host).await;
        assert!(pending.ok);
        let items = pending.data["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["status"], "delivered");

        std::env::remove_var("MUNDUS_TEST_LOCAL_DICTATION_TRANSCRIPT");
        std::env::remove_var("MUNDUS_TEST_MODE");
    }

    // KOS-337: exercises the local backend — only built with `local-dictation`.
    #[cfg(feature = "local-dictation")]
    #[tokio::test]
    async fn submit_audio_local_missing_model_path_returns_error_and_keeps_pending() {
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.local_model = Some("whisper-base".into());
        cfg.local_model_path = None;
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);

        let start = handle_dictation_op("start_recording", Value::Null, &host).await;
        assert!(start.ok, "start_recording failed: {:?}", start.error);

        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 2.0 }),
            &host,
        )
        .await;
        assert!(resp.ok, "submit_audio failed: {:?}", resp.error);
        assert_eq!(resp.data["state"], "error");
        assert!(
            resp.data["error"]
                .as_str()
                .unwrap_or_default()
                .contains("Локальная модель"),
            "error: {}",
            resp.data["error"]
        );

        let pending = op_list_pending(&host).await;
        assert!(pending.ok);
        assert_eq!(pending.data["items"].as_array().unwrap().len(), 1);
    }

    // KOS-337: exercises the local backend — only built with `local-dictation`.
    #[cfg(feature = "local-dictation")]
    #[tokio::test]
    async fn submit_audio_local_sidecar_unavailable_keeps_pending() {
        let _guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.local_model = Some("whisper-base".into());
        add_local_whisper_cpp_files(td.path(), &mut cfg);
        let host = DictationHost::new_for_test(
            td.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        {
            let mut state = host.state.lock().await;
            state.name = DictationStateName::Recording;
        }
        local::install_test_sidecar_unavailable_for_host(2);

        let wav = make_wav();
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav);
        let resp = handle_dictation_op(
            "submit_audio",
            json!({ "audioB64": audio_b64, "durationSec": 2.0 }),
            &host,
        )
        .await;

        local::clear_test_sidecar_mock_for_host();
        assert!(resp.ok, "submit_audio failed: {:?}", resp.error);
        assert_eq!(resp.data["state"], "idle");
        assert_eq!(resp.data["queued"], true);

        let pending = op_list_pending(&host).await;
        assert!(pending.ok);
        assert_eq!(pending.data["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unknown_subop_errors() {
        let (_data, host) = isolated_host();
        let resp = handle_dictation_op("nope", Value::Null, &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap_or_default().contains("unknown"));
    }

    #[test]
    fn platform_engine_policy_maps_windows_to_whisper_cpp() {
        assert_eq!(
            platform_local_engine_for_os("windows"),
            DEFAULT_LOCAL_ENGINE
        );
        assert_eq!(platform_local_engine_for_os("macos"), DEFAULT_LOCAL_ENGINE);
    }

    #[test]
    fn normalize_preserves_parakeet_engine_and_selection() {
        // Regression: normalize_platform_local_engine раньше затирал выбор
        // Parakeet (movie/path) на каждом старте/update, отключая provider.
        let mut cfg = DictationConfig {
            local_engine: local::PARAKEET_LOCAL_ENGINE.into(),
            local_model: Some("parakeet-tdt-0.6b-v3".into()),
            local_model_path: Some("C:/models/parakeet".into()),
            ..Default::default()
        };
        let changed = normalize_platform_local_engine(&mut cfg);
        assert!(!changed, "parakeet engine must not be normalized away");
        assert_eq!(cfg.local_engine, local::PARAKEET_LOCAL_ENGINE);
        assert_eq!(cfg.local_model.as_deref(), Some("parakeet-tdt-0.6b-v3"));
        assert_eq!(cfg.local_model_path.as_deref(), Some("C:/models/parakeet"));
    }

    #[test]
    fn normalize_migrates_unknown_legacy_engine() {
        let mut cfg = DictationConfig {
            local_engine: "faster-whisper".into(),
            local_model: Some("legacy".into()),
            local_model_path: Some("C:/legacy".into()),
            ..Default::default()
        };
        let changed = normalize_platform_local_engine(&mut cfg);
        assert!(changed, "unknown legacy engine must be migrated");
        assert_eq!(cfg.local_engine, DEFAULT_LOCAL_ENGINE);
        assert!(cfg.local_model.is_none());
        assert!(cfg.local_model_path.is_none());
    }

    #[tokio::test]
    async fn update_config_persists_and_emits_event() {
        let _dictation_guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let model_path = tmp.path().join("local-whisper.bin");
        let command_path = tmp.path().join("whisper-cli.exe");
        std::fs::write(&model_path, b"model").expect("model file");
        std::fs::write(&command_path, b"command").expect("command file");

        let host = DictationHost::new_for_test(
            tmp.path().into(),
            groq::GROQ_ENDPOINT.into(),
            DictationConfig::default(),
        );
        let mut rx = host.subscribe();
        let resp = handle_dictation_op(
            "update_config",
            json!({
                "language": "auto",
                "injectMode": "clipboard_only",
                "pillStyle": "compact",
                "provider": "local",
                "duckAudioDuringRecording": true,
                "localEngine": "whisper.cpp",
                "localModelPath": model_path.to_string_lossy(),
                "localCommandPath": command_path.to_string_lossy(),
                "localModelId": "whisper-base"
            }),
            &host,
        )
        .await;
        assert!(resp.ok, "update_config failed: {:?}", resp.error);

        let evt = tokio::time::timeout(std::time::Duration::from_secs(60), rx.recv())
            .await
            .expect("event timeout")
            .expect("event recv");
        assert_eq!(evt["event"], "dictation_config_changed");

        // Re-load: новый host подхватит persisted config.
        let config_path = tmp.path().join("dictation-config.json");
        assert!(config_path.is_file());
        let backup_path = tmp.path().join("dictation-config.json.bak");
        for path in [&config_path, &backup_path] {
            let bytes = std::fs::read(path).expect("persisted config");
            serde_json::from_slice::<DictationConfig>(&bytes).expect("valid config JSON");
        }
        let persisted = config::load_from(&config_path);
        let host2 =
            DictationHost::new_for_test(tmp.path().into(), groq::GROQ_ENDPOINT.into(), persisted);
        let state = handle_dictation_op("get_state", Value::Null, &host2).await;
        assert_eq!(state.data["config"]["language"], "auto");
        assert_eq!(state.data["config"]["injectMode"], "clipboard_only");
        assert_eq!(state.data["config"]["pillStyle"], "compact");
        assert_eq!(state.data["config"]["provider"], "local");
        assert_eq!(state.data["config"]["duckAudioDuringRecording"], true);
        assert_eq!(state.data["config"]["localEngine"], "whisper.cpp");
        assert_eq!(
            state.data["config"]["localModelPath"],
            model_path.to_string_lossy().as_ref()
        );
        assert_eq!(
            state.data["config"]["localCommandPath"],
            command_path.to_string_lossy().as_ref()
        );
        assert_eq!(state.data["config"]["localModel"], "whisper-base");
        assert_eq!(state.data["config"]["localModelId"], "whisper-base");
    }

    #[tokio::test]
    async fn production_host_loads_config_and_stats_from_explicit_data_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let cfg = DictationConfig {
            hotkey: "Ctrl+Shift+9".into(),
            ..Default::default()
        };
        config::save_to(&tmp.path().join("dictation-config.json"), &cfg).expect("save config");
        stats::save_to(
            &tmp.path().join("dictation-stats.json"),
            &DictationStats {
                total_words: 12,
                total_record_seconds: 4,
                total_sessions: 1,
            },
        )
        .expect("save stats");

        let host = DictationHost::new(tmp.path().into());
        assert_eq!(host.config.lock().await.hotkey, "Ctrl+Shift+9");
        assert_eq!(host.stats.lock().await.total_words, 12);
    }

    #[tokio::test]
    async fn update_config_unloads_local_sidecar_when_provider_switches_away() {
        let _dictation_guard = local::TEST_SIDECAR_TEST_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().expect("tempdir");

        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.provider_enabled = true;
        let host = DictationHost::new_for_test(
            tmp.path().into(),
            "http://127.0.0.1:1/openai/v1/audio/transcriptions".into(),
            cfg,
        );
        local::install_test_sidecar_mock_for_host(None);

        let resp = handle_dictation_op("update_config", json!({ "provider": "groq" }), &host).await;
        assert!(resp.ok, "update_config failed: {:?}", resp.error);

        let mut ops = Vec::new();
        for _ in 0..20 {
            ops = local::recorded_test_sidecar_ops_for_host();
            if ops.iter().any(|op| op == "unload") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        local::clear_test_sidecar_mock_for_host();
        assert!(ops.iter().any(|op| op == "unload"), "ops: {ops:?}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn local_models_list_and_use_downloaded_model_updates_config() {
        let tmp = tempfile::TempDir::new().expect("tempdir");

        let host = DictationHost::new_for_test(
            tmp.path().into(),
            groq::GROQ_ENDPOINT.into(),
            DictationConfig::default(),
        );
        let list = handle_dictation_op("list_local_models", Value::Null, &host).await;
        assert!(list.ok, "list_local_models failed: {:?}", list.error);
        assert!(list.data["models"].as_array().unwrap().len() >= 4);
        assert_eq!(list.data["commandInstalled"], false);

        let small = local_models::MODEL_CATALOG
            .iter()
            .find(|model| model.id == "small")
            .expect("small model in catalog");
        let model_path = local_models::model_path(tmp.path(), small);
        std::fs::create_dir_all(model_path.parent().unwrap()).expect("model dir");
        std::fs::write(&model_path, b"fake model").expect("model file");

        let command_path = local_models::command_path(tmp.path()).expect("windows command path");
        std::fs::create_dir_all(command_path.parent().unwrap()).expect("command dir");
        std::fs::write(&command_path, b"fake exe").expect("command file");

        let used =
            handle_dictation_op("use_local_model", json!({ "modelId": "small" }), &host).await;
        assert!(used.ok, "use_local_model failed: {:?}", used.error);
        assert_eq!(used.data["config"]["provider"], "local");
        assert_eq!(used.data["config"]["localEngine"], "whisper.cpp");
        assert_eq!(used.data["config"]["localModelId"], "small");
        assert_eq!(
            used.data["config"]["localModelPath"].as_str(),
            Some(model_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            used.data["config"]["localCommandPath"].as_str(),
            Some(command_path.to_string_lossy().as_ref())
        );
        assert_eq!(used.data["localModels"]["commandInstalled"], true);
        assert!(used.data["localModels"]["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|model| model["id"] == "small"
                && model["downloaded"] == true
                && model["selected"] == true));
    }

    #[tokio::test]
    async fn local_models_use_parakeet_directory_without_whisper_command() {
        let tmp = tempfile::TempDir::new().expect("tempdir");

        let host = DictationHost::new_for_test(
            tmp.path().into(),
            groq::GROQ_ENDPOINT.into(),
            DictationConfig::default(),
        );
        let parakeet = local_models::MODEL_CATALOG
            .iter()
            .find(|model| model.id == "parakeet-tdt-0.6b-v3")
            .expect("parakeet model in catalog");
        let model_path = local_models::model_path(tmp.path(), parakeet);
        std::fs::create_dir_all(&model_path).expect("model dir");

        let used = handle_dictation_op(
            "use_local_model",
            json!({ "modelId": "parakeet-tdt-0.6b-v3" }),
            &host,
        )
        .await;
        assert!(used.ok, "use_local_model failed: {:?}", used.error);
        assert_eq!(used.data["config"]["provider"], "local");
        assert_eq!(used.data["config"]["localEngine"], "parakeet");
        assert_eq!(used.data["config"]["localModelId"], "parakeet-tdt-0.6b-v3");
        assert_eq!(used.data["config"]["localCommandPath"], Value::Null);
        assert!(used.data["localModels"]["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|model| model["id"] == "parakeet-tdt-0.6b-v3"
                && model["downloaded"] == true
                && model["selected"] == true
                && model["transcriptionSupported"] == true));
    }

    // ---- autoselect_local_model ----

    fn fake_model(data_dir: &std::path::Path, id: &str) -> std::path::PathBuf {
        let spec = local_models::MODEL_CATALOG
            .iter()
            .find(|model| model.id == id)
            .expect("catalog model");
        let path = local_models::model_path(data_dir, spec);
        if spec.directory {
            std::fs::create_dir_all(&path).expect("model dir");
        } else {
            std::fs::create_dir_all(path.parent().unwrap()).expect("models dir");
            std::fs::write(&path, b"fake model").expect("model file");
        }
        path
    }

    #[cfg(windows)]
    fn fake_whisper_command(data_dir: &std::path::Path) -> std::path::PathBuf {
        let path = local_models::command_path(data_dir).expect("command path");
        std::fs::create_dir_all(path.parent().unwrap()).expect("command dir");
        std::fs::write(&path, b"fake exe").expect("command file");
        path
    }

    fn local_cfg() -> DictationConfig {
        let mut cfg = test_cfg();
        cfg.provider = "local".into();
        cfg.provider_enabled = true;
        cfg
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_picks_recommended_downloaded_model() {
        let td = tempfile::TempDir::new().unwrap();
        let small = fake_model(td.path(), "small");
        fake_model(td.path(), "turbo");
        let command = fake_whisper_command(td.path());
        let mut cfg = local_cfg(); // enabled, nothing selected
        assert!(autoselect_local_model(td.path(), &mut cfg));
        assert!(cfg.provider_enabled);
        assert_eq!(cfg.local_model.as_deref(), Some("small"));
        assert_eq!(cfg.local_engine, DEFAULT_LOCAL_ENGINE);
        assert_eq!(
            cfg.local_model_path.as_deref(),
            Some(small.to_string_lossy().as_ref())
        );
        assert_eq!(
            cfg.local_command_path.as_deref(),
            Some(command.to_string_lossy().as_ref())
        );
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_recovers_deleted_selection() {
        let td = tempfile::TempDir::new().unwrap();
        let small = fake_model(td.path(), "small");
        fake_whisper_command(td.path());
        let mut cfg = local_cfg();
        cfg.local_model = Some("turbo".into());
        cfg.local_model_path = Some(td.path().join("gone.bin").to_string_lossy().into_owned());
        assert!(autoselect_local_model(td.path(), &mut cfg));
        assert_eq!(cfg.local_model.as_deref(), Some("small"));
        assert_eq!(
            cfg.local_model_path.as_deref(),
            Some(small.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn autoselect_does_nothing_when_nothing_is_downloaded() {
        let td = tempfile::TempDir::new().unwrap();
        let mut cfg = local_cfg();
        let before = cfg.clone();
        assert!(!autoselect_local_model(td.path(), &mut cfg));
        assert_eq!(cfg.provider, before.provider);
        assert_eq!(cfg.provider_enabled, before.provider_enabled);
        assert_eq!(cfg.local_model, before.local_model);
        // The caller's fallback still applies: no downloaded model → the
        // provider is disabled and the UI reports the download-required
        // message (LOCAL_MODEL_NOT_READY_MSG), not a silent dead end.
        assert!(!local_config_is_ready(td.path(), &cfg));
        clear_local_selection(&mut cfg);
        assert!(!cfg.provider_enabled);
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_falls_back_to_catalog_order() {
        let td = tempfile::TempDir::new().unwrap();
        // No recommended model downloaded — catalog order wins ("turbo"
        // sits before "large").
        let turbo = fake_model(td.path(), "turbo");
        fake_model(td.path(), "large");
        fake_whisper_command(td.path());
        let mut cfg = local_cfg();
        assert!(autoselect_local_model(td.path(), &mut cfg));
        assert_eq!(cfg.local_model.as_deref(), Some("turbo"));
        assert_eq!(
            cfg.local_model_path.as_deref(),
            Some(turbo.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn autoselect_picks_parakeet_without_whisper_runtime() {
        let td = tempfile::TempDir::new().unwrap();
        // Whisper models are unusable without the runtime — a directory
        // model with its own engine is picked instead.
        fake_model(td.path(), "turbo");
        fake_model(td.path(), "parakeet-tdt-0.6b-v3");
        let mut cfg = local_cfg();
        assert!(autoselect_local_model(td.path(), &mut cfg));
        assert_eq!(cfg.local_model.as_deref(), Some("parakeet-tdt-0.6b-v3"));
        assert_eq!(cfg.local_engine, "parakeet");
        assert_eq!(cfg.local_command_path, None);
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_respects_explicitly_disabled_provider() {
        let td = tempfile::TempDir::new().unwrap();
        fake_model(td.path(), "small");
        fake_whisper_command(td.path());
        // providerEnabled=false with a surviving local_model is the user's
        // own off-switch — auto-select must not re-enable it.
        let mut cfg = local_cfg();
        cfg.provider_enabled = false;
        cfg.local_model = Some("small".into());
        cfg.local_model_path = Some(td.path().join("gone.bin").to_string_lossy().into_owned());
        let before = cfg.clone();
        assert!(!autoselect_local_model(td.path(), &mut cfg));
        assert!(!cfg.provider_enabled);
        assert_eq!(cfg.local_model, before.local_model);
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_recovers_cleared_selection() {
        let td = tempfile::TempDir::new().unwrap();
        fake_model(td.path(), "small");
        fake_whisper_command(td.path());
        // The post-clear/migration state the smoke report hit:
        // providerEnabled=false AND localModel=null is not a user choice.
        let mut cfg = local_cfg();
        cfg.provider_enabled = false;
        cfg.local_model = None;
        assert!(autoselect_local_model(td.path(), &mut cfg));
        assert!(cfg.provider_enabled);
        assert_eq!(cfg.local_model.as_deref(), Some("small"));
    }

    #[cfg(windows)]
    #[test]
    fn autoselect_ignores_non_local_provider() {
        let td = tempfile::TempDir::new().unwrap();
        fake_model(td.path(), "small");
        fake_whisper_command(td.path());
        let mut cfg = local_cfg();
        cfg.provider = "groq".into();
        cfg.local_model = Some("missing".into());
        cfg.local_model_path = Some(td.path().join("gone.bin").to_string_lossy().into_owned());
        let before = cfg.clone();
        assert!(!autoselect_local_model(td.path(), &mut cfg));
        assert_eq!(cfg.provider, before.provider);
        assert_eq!(cfg.local_model, before.local_model);
    }
