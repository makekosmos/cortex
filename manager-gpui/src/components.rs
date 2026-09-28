//! Sibling packaged components — the Windows installer stages GPUI apps next
//! to this exe as `resources/components/<name>/<Packaged>.exe`. The shared
//! `mundus_gpui_kit::engine` carries the Agenda pair (KOS-137); Memoria
//! (KOS-156) mirrors the same contract here without a kit bump.
use std::path::PathBuf;

/// Packaged Memoria GPUI lives next to this exe as
/// `resources/components/memoria/Memoria.exe` (KOS-156).
/// `MUNDUS_MEMORIA_EXECUTABLE` overrides for dev/local runs.
pub fn memoria_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MUNDUS_MEMORIA_EXECUTABLE").filter(|v| !v.is_empty()) {
        let candidate = PathBuf::from(path);
        return candidate.is_file().then_some(candidate);
    }
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.parent()?.join("memoria").join("Memoria.exe");
    candidate.is_file().then_some(candidate)
}

/// Launch the sibling Memoria component; the child inherits this process env
/// (the shell sets MUNDUS_DATA_DIR at spawn). `data_dir` re-pins the same
/// Engine lock when Manager itself was started directly.
pub fn open_memoria(data_dir: Option<&std::path::Path>) -> Result<(), String> {
    let exe = memoria_executable().ok_or("Memoria не входит в эту сборку Mundus.")?;
    let mut command = std::process::Command::new(exe);
    if let Some(dir) = data_dir {
        command.env("MUNDUS_DATA_DIR", dir);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Не удалось запустить Memoria: {e}"))
}
