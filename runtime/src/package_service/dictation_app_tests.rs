// Every test here exercises process spawn/kill semantics that only exist on
// Windows; gating each test left dead helpers and unused imports on macOS.
#[cfg(all(test, windows))]
mod dictation_app_tests {
    use super::*;
    use crate::native_apps::{NativeAppStore, NativeInstallSpec};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    fn service(dir: &tempfile::TempDir) -> PackageService {
        PackageService::from_parts(dir.path().join("packages"), Some(dir.path().join("apps")))
            .expect("service")
    }

    fn dictation() -> &'static NativeAppDescriptor {
        crate::native_apps::app_descriptor(DICTATION_APP_ID).unwrap()
    }

    /// Write a fake install record + exe so the store resolves a launchable
    /// executable without any network or real release.
    fn install_fake(dir: &tempfile::TempDir) -> PathBuf {
        let exe = dictation().executable(crate::native_apps::host_app_target().unwrap());
        let archive = dir.path().join("dictation.zip");
        {
            let file = std::fs::File::create(&archive).expect("zip file");
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file(&exe, zip::write::FileOptions::default())
                .expect("exe entry");
            use std::io::Write;
            zip.write_all(b"MZ test fixture").expect("exe write");
            zip.finish().expect("zip finish");
        }
        let store = NativeAppStore::new(dir.path().join("apps")).expect("store");
        let bytes = std::fs::read(&archive).expect("archive bytes");
        store
            .install_archive(
                &NativeInstallSpec {
                    id: DICTATION_APP_ID.into(),
                    version: "0.4.0".into(),
                    executable: exe,
                    sha256: format!("{:x}", Sha256::digest(&bytes)),
                    size: bytes.len() as u64,
                    repository: dictation().repository.into(),
                    release_tag: dictation().release_tag("0.4.0"),
                },
                &archive,
            )
            .expect("install");
        dir.path()
            .join("apps")
            .join(DICTATION_APP_ID)
            .join("0.4.0")
            .join(dictation().executable(crate::native_apps::host_app_target().unwrap()))
    }

    /// Hold the exe open for write with share=read-only — the running-image
    /// probe (`exe_in_use`) sees exactly what a live process looks like.
    #[cfg(windows)]
    fn simulate_running(exe: &Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0x0000_0001) // FILE_SHARE_READ only
            .open(exe)
            .expect("lock exe")
    }

    #[cfg(windows)]
    #[test]
    fn ensure_running_spawns_installed_app_in_background() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let exe = install_fake(&dir);
        let spawned = std::sync::Mutex::new(Vec::<PathBuf>::new());
        service.ensure_dictation_running_with(|path| {
            spawned.lock().unwrap().push(path.to_path_buf());
            Ok(())
        });
        assert_eq!(spawned.lock().unwrap().as_slice(), &[exe]);
    }

    #[cfg(windows)]
    #[test]
    fn ensure_running_is_noop_when_not_installed() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let calls = AtomicUsize::new(0);
        service.ensure_dictation_running_with(|_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[cfg(windows)]
    #[test]
    fn ensure_running_skips_spawn_when_already_running() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let exe = install_fake(&dir);
        let _running = simulate_running(&exe);
        let calls = AtomicUsize::new(0);
        service.ensure_dictation_running_with(|_| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[cfg(windows)]
    #[test]
    fn stop_waits_for_exit_and_unblocks_mutation() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let exe = install_fake(&dir);
        let running = std::sync::Mutex::new(Some(simulate_running(&exe)));
        let terminated = AtomicUsize::new(0);
        assert!(service.stop_dictation_app_with(
            |_| {
                terminated.fetch_add(1, Ordering::SeqCst);
                // The kill went out; the process exits → lock released.
                running.lock().unwrap().take();
                Ok(())
            },
            Duration::from_secs(1),
        ));
        assert_eq!(terminated.load(Ordering::SeqCst), 1);
        assert!(service.uninstall_native_app(dictation()).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn stop_fails_when_process_survives() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let exe = install_fake(&dir);
        let _running = simulate_running(&exe);
        assert!(!service.stop_dictation_app_with(
            |_| Ok(()), // terminate claims success but the process stays
            Duration::from_millis(200),
        ));
        // ...and the Store-level mutation still refuses a live exe.
        let store = service.native_store().expect("store");
        assert!(matches!(
            store.uninstall(DICTATION_APP_ID),
            Err(crate::native_apps::NativeAppError::Running)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn stop_fails_when_terminate_errors() {
        let dir = tempdir().unwrap();
        let service = service(&dir);
        let exe = install_fake(&dir);
        let _running = simulate_running(&exe);
        assert!(!service.stop_dictation_app_with(
            |_| Err(std::io::Error::other("denied")),
            Duration::from_millis(50),
        ));
    }
}
