use super::install::extract_whisper_runtime;
use super::*;
use crate::local_models::command_path;

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
    fs::write(path, b"exe").expect("file");
}

#[test]
fn command_path_prefers_installed_vulkan_over_cpu() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    touch(&cpu_command_path(tmp.path()));
    touch(&vulkan_command_path(tmp.path()));

    assert!(same_path_or_text(
        &command_path(tmp.path()).expect("command"),
        &vulkan_command_path(tmp.path())
    ));
}

#[test]
fn whisper_cpp_runtime_sources_are_pinned() {
    assert!(WHISPER_CPP_CPU_ZIP_URL
        .contains("github.com/makekosmos/engine-addons/releases/download/runtime-v1.9.3"));
    assert!(WHISPER_CPP_CPU_ZIP_URL.contains("whisper-cpu-bin-x64-v1.9.3"));
    assert_eq!(WHISPER_CPP_CPU_ZIP_SHA256.len(), 64);
    assert_eq!(WHISPER_CPP_CPU_ZIP_SIZE, 1_436_012);
    assert!(WHISPER_CPP_VULKAN_ZIP_URL
        .contains("github.com/makekosmos/engine-addons/releases/download/runtime-v1.9.3"));
    assert!(WHISPER_CPP_VULKAN_ZIP_URL.contains("whisper-vulkan-bin-x64-v1.9.3"));
    assert_eq!(WHISPER_CPP_VULKAN_ZIP_SHA256.len(), 64);
    assert_eq!(WHISPER_CPP_VULKAN_ZIP_SIZE, 17_403_912);
}

#[test]
fn runtime_reuse_rejects_tampered_files() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tools_dir(tmp.path());
    for name in runtime_file_names(false) {
        touch(&dir.join(name));
    }
    fs::write(
        dir.join(".runtime-version"),
        format!("{WHISPER_RUNTIME_VERSION}\n"),
    )
    .expect("version");
    write_runtime_integrity(&dir, false).expect("integrity");
    assert!(runtime_is_current(&dir));
    fs::write(dir.join("Release/whisper-cli.exe"), b"tampered").expect("tamper");
    assert!(!runtime_is_current(&dir));
}

#[test]
fn interrupted_runtime_swap_recovers_previous_install() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let dir = tools_dir(tmp.path());
    let parent = dir.parent().expect("parent");
    let stem = dir.file_name().expect("stem").to_str().expect("stem");
    let previous = parent.join(format!(".{stem}.previous"));
    let staging = parent.join(format!(".{stem}.staging"));
    fs::create_dir_all(previous.join("Release")).expect("previous");
    fs::create_dir_all(&staging).expect("staging");
    write_runtime_transaction(&dir).expect("transaction");
    recover_runtime_transaction(&dir).expect("recovery");
    assert!(dir.join("Release").is_dir());
    assert!(!previous.exists());
    assert!(!staging.exists());
    assert!(!runtime_transaction_path(&dir).exists());
}

#[tokio::test]
#[ignore = "requires the immutable production runtime release"]
async fn pinned_runtime_release_installs_atomically() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let client = Client::new();
    install_whisper_cpp_zip(
        &client,
        &tools_dir(tmp.path()),
        "whisper-cpu-bin-x64.zip",
        WHISPER_CPP_CPU_ZIP_URL,
        WHISPER_CPP_CPU_ZIP_SHA256,
        WHISPER_CPP_CPU_ZIP_SIZE,
        false,
        &mut |_| {},
    )
    .await
    .expect("pinned CPU runtime install");
    assert!(cpu_command_path(tmp.path()).is_file());
    assert!(runtime_is_current(&tools_dir(tmp.path())));

    let command = ensure_whisper_cpp(&client, tmp.path())
        .await
        .expect("pinned Vulkan runtime install");

    assert!(command.is_file());
    assert!(vulkan_server_path(tmp.path()).is_file());
    assert!(runtime_is_current(&vulkan_tools_dir(tmp.path())));
}

#[test]
fn runtime_extractor_rejects_traversal_before_writing() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let archive_path = tmp.path().join("runtime.zip");
    let file = fs::File::create(&archive_path).expect("archive");
    let mut archive = zip::ZipWriter::new(file);
    for name in [
        "../LICENSE.whisper.cpp.txt",
        "Release/ggml-base.dll",
        "Release/ggml-cpu.dll",
        "Release/ggml.dll",
        "Release/whisper-cli.exe",
        "Release/whisper-server.exe",
        "Release/whisper.dll",
    ] {
        archive
            .start_file(name, zip::write::FileOptions::default())
            .expect("entry");
        archive.write_all(b"x").expect("body");
    }
    archive.finish().expect("finish");
    let destination = tmp.path().join("staging");
    fs::create_dir(&destination).expect("staging");

    let error = extract_whisper_runtime(&archive_path, &destination, false)
        .expect_err("traversal must fail");

    assert!(error.to_string().contains("unsafe runtime archive"));
    assert!(!tmp.path().join("LICENSE.whisper.cpp.txt").exists());
}

#[test]
fn refresh_managed_command_path_updates_cpu_to_vulkan() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    touch(&cpu_command_path(tmp.path()));
    touch(&vulkan_command_path(tmp.path()));
    let mut cfg = config::DictationConfig {
        local_command_path: Some(path_string(&cpu_command_path(tmp.path()))),
        ..Default::default()
    };

    assert!(refresh_managed_command_path(tmp.path(), &mut cfg));
    assert_eq!(
        cfg.local_command_path.as_deref(),
        Some(path_string(&vulkan_command_path(tmp.path())).as_str())
    );
}
