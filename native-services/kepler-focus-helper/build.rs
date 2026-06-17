#[cfg(windows)]
fn main() {
    use embed_manifest::manifest::ExecutionLevel;
    use embed_manifest::{embed_manifest as do_embed, embed_manifest_file, new_manifest};
    // Сначала пробуем embed нашего ручного манифеста; если файл не найден
    // (build из другого cwd), fallback на synthesized manifest с
    // requireAdministrator.
    if std::path::Path::new("app.manifest").exists() {
        embed_manifest_file("app.manifest").expect("embed app.manifest");
    } else {
        do_embed(
            new_manifest("Kosmos.Helper")
                .requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("embed synthesized manifest");
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(not(windows))]
fn main() {}
