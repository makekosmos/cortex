//! Measure only product-owned roots outside the active data directory.
use super::{classify, classify_models_entry, directory_bytes, Category};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn add_root(value: &mut Value, path: &Path, category: Category, counted: &mut Vec<PathBuf>) {
    let Ok(root) = std::fs::canonicalize(path) else {
        return;
    };
    if counted.iter().any(|parent| root.starts_with(parent)) {
        return;
    }
    // Do not count a parent that contains the active data root either.
    if counted.iter().any(|child| child.starts_with(&root)) {
        return;
    }
    let bytes = if root.is_dir() {
        directory_bytes(&root)
    } else {
        std::fs::metadata(&root).map(|m| m.len()).unwrap_or(0)
    };
    counted.push(root.clone());
    if bytes == 0 {
        return;
    }
    let Some(categories) = value["categories"].as_array_mut() else {
        return;
    };
    let index = categories.iter().position(|c| c["id"] == category.id());
    let index = index.unwrap_or_else(|| {
        categories.push(json!({"id": category.id(), "label": category.label(), "bytes": 0}));
        categories.len() - 1
    });
    let entry = &mut categories[index];
    entry["bytes"] = json!(entry["bytes"].as_u64().unwrap_or(0).saturating_add(bytes));
    if !entry["detail"].is_array() {
        entry["detail"] = json!([]);
    }
    if let Some(detail) = entry["detail"].as_array_mut() {
        detail.push(json!({"path": root, "bytes": bytes}));
    }
    value["total_bytes"] = json!(value["total_bytes"]
        .as_u64()
        .unwrap_or(0)
        .saturating_add(bytes));
}

pub(super) fn add_external_storage(value: &mut Value, data_dir: &Path, package_root: &Path) {
    let mut counted =
        vec![std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf())];
    add_root(value, package_root, Category::Packages, &mut counted);
    let roots = [
        crate::data_dir::mundus_config_dir().ok(),
        crate::data_dir::mundus_local_dir(),
    ];
    for local in roots.into_iter().flatten() {
        // On Windows roaming and local roots differ; on macOS this is already counted.
        if let Ok(entries) = std::fs::read_dir(local) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == "models" && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    if let Ok(models) = std::fs::read_dir(entry.path()) {
                        for model in models.flatten() {
                            add_root(
                                value,
                                &model.path(),
                                classify_models_entry(&model.file_name().to_string_lossy()),
                                &mut counted,
                            );
                        }
                    }
                    continue;
                }
                let category = classify(&name, false);
                // Only the installed roaming ARK database is represented in БД.
                if !matches!(category, Category::Database) {
                    add_root(value, &entry.path(), category, &mut counted);
                }
            }
        }
    }
    if let Ok(config) = crate::data_dir::mundus_config_dir() {
        if let Some(parent) = config.parent() {
            add_root(
                value,
                &parent.join("Mundus Manager"),
                Category::Settings,
                &mut counted,
            );
        }
    }
    add_model_assets(value, data_dir, &mut counted);
    #[cfg(target_os = "macos")]
    {
        add_root(
            value,
            Path::new("/Applications/Mundus Manager.app"),
            Category::Installations,
            &mut counted,
        );
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            add_root(
                value,
                &home.join("Applications/Mundus Manager.app"),
                Category::Installations,
                &mut counted,
            );
            add_root(
                value,
                &home.join("Library/Caches/Mundus"),
                Category::Cache,
                &mut counted,
            );
        }
    }
}

fn add_model_assets(value: &mut Value, data_dir: &Path, counted: &mut Vec<PathBuf>) {
    let assets = crate::dictation::local_models::default_shared_assets_root();
    let models = assets.join("models/dictation");
    if let Ok(entries) = std::fs::read_dir(models) {
        for entry in entries.flatten() {
            add_root(value, &entry.path(), Category::DictationModels, counted);
        }
    }
    let tools = assets.join("tools/dictation/whisper.cpp");
    if let Some(parent) = tools.parent() {
        add_root(value, parent, Category::Tools, counted);
    }
    for root in [
        Some(data_dir.to_path_buf()),
        crate::data_dir::mundus_config_dir().ok(),
    ]
    .into_iter()
    .flatten()
    {
        let config = crate::dictation::config::load_from(&root.join("dictation-config.json"));
        if let Some(path) = config
            .local_model_path
            .filter(|path| !path.trim().is_empty())
        {
            add_root(value, Path::new(&path), Category::DictationModels, counted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_roots_count_once_and_keep_total_consistent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("app")).unwrap();
        std::fs::write(root.join("app/binary"), [0; 64]).unwrap();
        let mut value = json!({"total_bytes": 10, "categories": [{"id":"database", "bytes":10}]});
        let mut counted = vec![root.join("data")];
        add_root(
            &mut value,
            &root.join("app"),
            Category::Installations,
            &mut counted,
        );
        add_root(
            &mut value,
            &root.join("app/binary"),
            Category::Installations,
            &mut counted,
        );
        add_root(&mut value, root, Category::Other, &mut counted);
        assert_eq!(value["total_bytes"], 74);
        assert_eq!(value["categories"][1]["bytes"], 64);
    }

    #[test]
    fn external_parakeet_assets_are_counted_and_named_without_duplicate_paths() {
        let dir = tempfile::tempdir().unwrap();
        let model = dir.path().join("parakeet-tdt-0.6b-v3-ultra-int8");
        std::fs::create_dir(&model).unwrap();
        std::fs::write(model.join("encoder-model.int8.onnx"), [0; 64]).unwrap();
        let mut value = json!({"total_bytes":0, "categories":[]});
        let mut counted = vec![];
        add_root(&mut value, &model, Category::DictationModels, &mut counted);
        add_root(&mut value, &model, Category::DictationModels, &mut counted);
        super::super::presentation::describe(&mut value);
        assert_eq!(value["total_bytes"], 64);
        assert_eq!(
            value["categories"][0]["detail"][0]["label"],
            "Parakeet Ultra"
        );
    }
}
