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
