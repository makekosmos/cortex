use super::legacy::merge_legacy_dir_into_shared;
use super::*;

#[test]
fn snapshot_does_not_mark_missing_selected_model() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let spec = MODEL_CATALOG
        .iter()
        .find(|model| model.id == "turbo")
        .expect("turbo model");
    let path = model_path(tmp.path(), spec);
    let cfg = config::DictationConfig {
        provider: "local".into(),
        provider_enabled: true,
        local_model: Some(spec.id.to_owned()),
        local_model_path: Some(path_string(&path)),
        ..Default::default()
    };

    let snapshot = snapshot(tmp.path(), &cfg);
    let model = snapshot
        .models
        .iter()
        .find(|model| model.id == spec.id)
        .expect("snapshot model");
    assert!(!model.downloaded);
    assert!(!model.selected);
}

#[test]
fn delete_model_ignores_backend_cleanup_failure_after_file_delete() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let spec = MODEL_CATALOG
        .iter()
        .find(|model| model.id == "small")
        .expect("small model");
    let path = model_path(tmp.path(), spec);
    fs::create_dir_all(path.parent().expect("model parent")).expect("model dir");
    fs::write(&path, b"model").expect("model file");
    fs::create_dir_all(tools_dir(tmp.path()).parent().expect("tools parent"))
        .expect("tools parent");
    fs::write(tools_dir(tmp.path()), b"not a directory").expect("cleanup blocker");

    let deleted = delete_model(tmp.path(), spec.id).expect("delete model");
    assert_eq!(deleted, path);
    assert!(!deleted.exists());
}

#[test]
fn legacy_merge_keeps_conflicting_files() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let legacy = tmp.path().join("legacy");
    let shared = tmp.path().join("shared");
    fs::create_dir_all(&legacy).expect("legacy dir");
    fs::create_dir_all(&shared).expect("shared dir");
    fs::write(legacy.join("ggml.bin"), b"old!").expect("legacy model");
    fs::write(shared.join("ggml.bin"), b"new!").expect("shared model");

    assert!(merge_legacy_dir_into_shared(&legacy, &shared).expect("merge"));
    assert_eq!(fs::read(shared.join("ggml.bin")).expect("shared"), b"new!");
    assert_eq!(
        fs::read(shared.join("ggml.bin.legacy-1")).expect("legacy copy"),
        b"old!"
    );
    assert!(!legacy.exists());
}

#[test]
fn delete_last_model_removes_unused_backend_dirs() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let small = model_spec("small").expect("small model");
    let model_path = model_path(tmp.path(), small);
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("model dir");
    fs::write(&model_path, b"model").expect("model file");
    fs::create_dir_all(tools_dir(tmp.path())).expect("tools dir");

    let deleted = delete_model(tmp.path(), "small").expect("delete model");

    assert_eq!(deleted, model_path);
    assert!(!tools_dir(tmp.path()).exists());
}

#[test]
fn extract_tar_gz_installs_directory_model() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let archive_path = tmp.path().join("model.tar.gz");
    let tar_gz = fs::File::create(&archive_path).expect("archive");
    let encoder = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    let body = b"config";
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "nested/config.json", &body[..])
        .expect("append tar");
    archive
        .into_inner()
        .expect("finish tar")
        .finish()
        .expect("gzip");

    let destination = tmp.path().join("parakeet-tdt-0.6b-v3-int8");
    extract_tar_gz(&archive_path, &destination).expect("extract");

    assert_eq!(
        fs::read(destination.join("config.json")).expect("extracted file"),
        body
    );
    assert!(!destination.with_extension("extracting").exists());
}

#[test]
fn cleanup_obsolete_local_stt_assets_deletes_ct2_cache_even_with_ggml_models() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let small = model_spec("small").expect("small model");
    let model_path = model_path(tmp.path(), small);
    fs::create_dir_all(model_path.parent().expect("model parent")).expect("model dir");
    fs::write(&model_path, b"ggml").expect("ggml model");

    let old_ct2 = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_MODEL_CACHE_DIR);
    let old_runtime = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_RUNTIME_DIR);
    let old_cuda = shared_assets_root(tmp.path()).join(OLD_FASTER_WHISPER_CUDA_LIBS_DIR);
    fs::create_dir_all(&old_ct2).expect("ct2 dir");
    fs::create_dir_all(&old_runtime).expect("runtime dir");
    fs::create_dir_all(&old_cuda).expect("cuda dir");

    assert!(cleanup_obsolete_local_stt_assets(tmp.path()).expect("cleanup"));
    assert!(model_path.is_file());
    assert!(!old_ct2.exists());
    assert!(!old_runtime.exists());
    assert!(!old_cuda.exists());
}
