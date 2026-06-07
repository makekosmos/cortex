#[cfg(windows)]
fn main() {
    use embed_manifest::manifest::ExecutionLevel;
    use embed_manifest::{embed_manifest as do_embed, embed_manifest_file, new_manifest};
    // Service runs under LocalSystem via SCM, не нужно UAC elevation.
    // Manifest = asInvoker — sub-commands (install/uninstall/...) запускаются
    // user-mode, и сами вернут needs_elevation:true когда нужны admin rights.
    if std::path::Path::new("app.manifest").exists() {
        embed_manifest_file("app.manifest").expect("embed app.manifest");
    } else {
        do_embed(
            new_manifest("Kosmos.SystemService")
                .requested_execution_level(ExecutionLevel::AsInvoker),
        )
        .expect("embed synthesized manifest");
    }
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(not(windows))]
fn main() {}
