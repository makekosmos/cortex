use super::{
    atomic_temp_path, backup_path, load_from, save_to, write_atomic, DictationConfig, InjectMode,
    NetworkProfile, TriggerMode,
};
use tempfile::TempDir;

#[test]
fn load_returns_default_when_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("missing.json");
    let cfg = load_from(&path);
    assert_eq!(cfg.hotkey, "Ctrl+Shift+;");
    assert_eq!(cfg.trigger_mode, TriggerMode::Toggle);
    assert_eq!(cfg.language, "ru");
    assert_eq!(cfg.inject_mode, InjectMode::AutoPaste);
}

#[test]
fn save_then_load_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let cfg = DictationConfig {
        hotkey: "Ctrl+Alt+D".into(),
        trigger_mode: TriggerMode::Toggle,
        language: "en".into(),
        inject_mode: InjectMode::ClipboardOnly,
        network_profile: NetworkProfile::CloudflareDoh,
        http_proxy: Some("http://127.0.0.1:8888".into()),
        transcription_prompt: "Mundus Groq".into(),
        provider: "groq".into(),
        provider_enabled: true,
        model: "whisper-large-v3".into(),
        local_engine: "whisper.cpp".into(),
        local_model_path: Some("C:/models/ggml-base.bin".into()),
        local_command_path: Some("C:/tools/whisper-cli.exe".into()),
        local_model: Some("ggml-base".into()),
        microphone_device_id: None,
        duck_audio_during_recording: true,
        local_idle_unload_ms: Some(60_000),
    };
    save_to(&path, &cfg).expect("save");
    let loaded = load_from(&path);
    assert_eq!(loaded.hotkey, "Ctrl+Alt+D");
    assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
    assert_eq!(loaded.network_profile, NetworkProfile::CloudflareDoh);
    assert_eq!(loaded.http_proxy.as_deref(), Some("http://127.0.0.1:8888"));
    assert_eq!(loaded.transcription_prompt, "Mundus Groq");
    assert_eq!(loaded.local_engine, "whisper.cpp");
    assert_eq!(
        loaded.local_model_path.as_deref(),
        Some("C:/models/ggml-base.bin")
    );
    assert_eq!(
        loaded.local_command_path.as_deref(),
        Some("C:/tools/whisper-cli.exe")
    );
    assert_eq!(loaded.local_model.as_deref(), Some("ggml-base"));
    assert!(loaded.duck_audio_during_recording);
    // Новое поле: round-trip.
    assert_eq!(loaded.local_idle_unload_ms, Some(60_000));
}

#[test]
fn local_idle_unload_ms_null_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let mut cfg = DictationConfig::default();
    cfg.local_idle_unload_ms = None; // "Не выгружать"
    save_to(&path, &cfg).expect("save");
    let loaded = load_from(&path);
    assert_eq!(loaded.local_idle_unload_ms, None);
}

#[test]
fn local_idle_unload_ms_default_on_legacy_config() {
    // Конфиг без localIdleUnloadMs должен загрузиться с дефолтом 5 минут.
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("legacy.json");
    std::fs::write(
        &path,
        concat!(
            r#"{"hotkey":"Ctrl+Shift+;","trigger_mode":"toggle","language":"ru","#,
            r#""inject_mode":"auto_paste","network_profile":{"kind":"system"},"#,
            r#""provider":"groq","model":"whisper-large-v3"}"#
        ),
    )
    .expect("write");
    let loaded = load_from(&path);
    assert_eq!(loaded.local_idle_unload_ms, Some(600_000));
}

#[test]
fn save_then_load_legacy_no_proxy_field() {
    // Конфиги созданные до Phase 1.5 не имеют http_proxy / transcription_prompt —
    // serde с `#[serde(default)]` должен дать defaults без ошибок.
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("legacy.json");
    std::fs::write(
        &path,
        concat!(
            r#"{"hotkey":"Ctrl+Shift+;","trigger_mode":"toggle","language":"ru","#,
            r#""inject_mode":"auto_paste","network_profile":{"kind":"system"},"#,
            r#""provider":"groq","model":"whisper-large-v3"}"#
        ),
    )
    .expect("write");
    let loaded = load_from(&path);
    assert_eq!(loaded.hotkey, "Ctrl+Shift+;");
    assert!(loaded.http_proxy.is_none());
    assert!(loaded.transcription_prompt.is_empty());
    assert_eq!(loaded.local_engine, "whisper.cpp");
    assert!(loaded.local_model_path.is_none());
    assert!(loaded.local_command_path.is_none());
    assert!(loaded.local_model.is_none());
}

#[test]
fn load_ignores_malformed_json() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("broken.json");
    std::fs::write(&path, "{not json").expect("write");
    let cfg = load_from(&path);
    assert_eq!(cfg.hotkey, "Ctrl+Shift+;");
}

#[test]
fn load_uses_backup_when_primary_is_malformed() {
    // Regression: 2026-06-23. Partial JSON writes must not reset all settings.
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let mut cfg = DictationConfig::default();
    cfg.hotkey = "Super+H".into();
    cfg.provider = "local".into();
    cfg.model = "whisper-large-v3-turbo".into();
    cfg.inject_mode = InjectMode::ClipboardOnly;
    save_to(&path, &cfg).expect("save");
    std::fs::write(&path, "{truncated").expect("corrupt primary");

    let loaded = load_from(&path);
    assert_eq!(loaded.hotkey, "Super+H");
    assert_eq!(loaded.provider, "local");
    assert_eq!(loaded.model, "whisper-large-v3-turbo");
    assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
}

#[test]
fn load_uses_backup_when_primary_is_missing() {
    // Regression: 2026-07-03. Updates must not reset hotkey if primary config disappears.
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let mut cfg = DictationConfig::default();
    cfg.hotkey = "Shift+PageUp".into();
    cfg.provider = "local".into();
    save_to(&path, &cfg).expect("save");
    std::fs::remove_file(&path).expect("remove primary");

    let loaded = load_from(&path);
    assert_eq!(loaded.hotkey, "Shift+PageUp");
    assert_eq!(loaded.provider, "local");
}

#[test]
fn lenient_recovery_preserves_hotkey_when_one_field_is_bad_typed() {
    // Regression: одно поле неверного типа (localIdleUnloadMs как строка)
    // не должно сбрасывать весь конфиг и терять пользовательский hotkey.
    // Backup отсутствует (нет .bak), поэтому strict primary + strict backup
    // оба фейлятся → срабатывает field-by-field recovery.
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    std::fs::write(
        &path,
        concat!(
            r#"{"hotkey":"Shift+PageUp","triggerMode":"toggle","provider":"local","#,
            r#""model":"whisper-large-v3-turbo","injectMode":"clipboard_only","#,
            r#""localIdleUnloadMs":"not-a-number"}"#
        ),
    )
    .expect("write");

    let loaded = load_from(&path);
    // Спасённые поля.
    assert_eq!(loaded.hotkey, "Shift+PageUp");
    assert_eq!(loaded.provider, "local");
    assert_eq!(loaded.model, "whisper-large-v3-turbo");
    assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
    // Битое поле падает на дефолт, остальное цело.
    assert_eq!(loaded.local_idle_unload_ms, Some(600_000));
}

#[test]
fn save_writes_backup_copy() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    save_to(&path, &DictationConfig::default()).expect("save");
    assert!(backup_path(&path).exists());
}

#[test]
fn atomic_temp_paths_are_unique_per_write() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    assert_ne!(atomic_temp_path(&path), atomic_temp_path(&path));
}

#[test]
fn concurrent_saves_leave_valid_files_and_no_temps() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for index in 0..8 {
            let path = &path;
            let barrier = &barrier;
            scope.spawn(move || {
                let cfg = DictationConfig {
                    hotkey: format!("Ctrl+Shift+{index}"),
                    ..Default::default()
                };
                barrier.wait();
                save_to(path, &cfg).expect("concurrent save");
            });
        }
    });

    for saved_path in [&path, &backup_path(&path)] {
        let bytes = std::fs::read(saved_path).expect("saved config");
        serde_json::from_slice::<DictationConfig>(&bytes).expect("valid config JSON");
    }
    assert!(std::fs::read_dir(tmp.path())
        .expect("read tempdir")
        .all(|entry| !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .contains(".tmp.")));
}

#[test]
fn failed_atomic_replace_removes_temp_file() {
    let tmp = TempDir::new().expect("tempdir");
    let destination = tmp.path().join("destination");
    std::fs::create_dir(&destination).expect("destination directory");
    assert!(write_atomic(&destination, b"config").is_err());
    assert!(std::fs::read_dir(tmp.path())
        .expect("read tempdir")
        .all(|entry| !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .contains(".tmp.")));
}

#[test]
fn save_replaces_existing_config() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    save_to(&path, &DictationConfig::default()).expect("first save");

    let cfg = DictationConfig {
        hotkey: "Ctrl+Shift+K".into(),
        inject_mode: InjectMode::ClipboardOnly,
        ..Default::default()
    };
    save_to(&path, &cfg).expect("second save");

    let loaded = load_from(&path);
    assert_eq!(loaded.hotkey, "Ctrl+Shift+K");
    assert_eq!(loaded.inject_mode, InjectMode::ClipboardOnly);
}

#[test]
fn custom_doh_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("cfg.json");
    let cfg = DictationConfig {
        network_profile: NetworkProfile::CustomDoh {
            url: "https://comss.dns.controld.com/dns-query".into(),
        },
        ..Default::default()
    };
    save_to(&path, &cfg).expect("save");
    let loaded = load_from(&path);
    match loaded.network_profile {
        NetworkProfile::CustomDoh { url } => {
            assert!(url.contains("comss"));
        }
        other => panic!("expected CustomDoh, got {other:?}"),
    }
}
