    fn minimal_pe() -> Vec<u8> {
        let mut bytes = vec![0; 68];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(64u32).to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes
    }

    #[test]
    fn launch_state_requires_containment_before_resume() {
        assert_eq!(advance_launch_state(LaunchState::CreatedSuspended), None);
        assert_eq!(
            advance_launch_state(LaunchState::JobConfigured),
            Some(LaunchState::Assigned)
        );
        assert_eq!(
            advance_launch_state(LaunchState::Assigned),
            Some(LaunchState::Resumed)
        );
        assert_eq!(advance_launch_state(LaunchState::Resumed), None);
    }

    #[test]
    fn handle_allowlist_byte_length_requires_nonzero_count() {
        let handle_size = size_of::<usize>();
        assert_eq!(
            checked_handle_allowlist_byte_len(3, handle_size),
            Some(3 * handle_size)
        );
        assert_eq!(checked_handle_allowlist_byte_len(0, handle_size), None);
    }

    #[test]
    fn handle_allowlist_byte_length_rejects_overflow() {
        assert_eq!(
            checked_handle_allowlist_byte_len(usize::MAX, size_of::<usize>()),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_handle_allowlist_byte_length_is_three_handles() {
        assert_eq!(
            handle_allowlist_byte_len(3),
            Some(3 * size_of::<windows::Win32::Foundation::HANDLE>())
        );
    }

    #[cfg(windows)]
    #[test]
    fn named_pipe_direction_matrix_is_complementary() {
        assert_eq!(child_pipe_access(false), (true, false));
        assert_eq!(child_pipe_access(true), (false, true));
    }

    #[cfg(windows)]
    #[test]
    fn startup_attribute_list_requires_extended_startup_flag() {
        let flags = CREATE_SUSPENDED
            | CREATE_NO_WINDOW
            | CREATE_UNICODE_ENVIRONMENT
            | EXTENDED_STARTUPINFO_PRESENT;
        assert_ne!(flags & EXTENDED_STARTUPINFO_PRESENT, 0);
        assert_eq!(
            flags & EXTENDED_STARTUPINFO_PRESENT,
            EXTENDED_STARTUPINFO_PRESENT
        );
    }

    #[test]
    fn rejects_relative_non_exe_and_missing_paths() {
        assert!(matches!(
            validate_executable(Path::new("worker.exe")),
            Err(WorkerProcessError::InvalidExecutable)
        ));
        assert!(matches!(
            validate_executable(Path::new("C:\\missing-worker.exe")),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[test]
    fn rejects_non_pe_even_with_exe_suffix() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("worker.exe");
        std::fs::write(&path, b"not a PE").expect("temporary executable");
        assert!(matches!(
            validate_executable(&path),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[test]
    fn rejects_pe_with_out_of_bounds_header_offset() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("worker.exe");
        let mut bytes = minimal_pe();
        bytes[0x3c..0x40].copy_from_slice(&1024u32.to_le_bytes());
        std::fs::write(&path, bytes).expect("temporary executable");
        assert!(matches!(
            validate_executable(&path),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }

    #[test]
    fn rejects_tampered_package_entrypoint_before_spawn() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let archive = directory.path().join("worker.kspkg");
        let manifest = crate::package_manifest::ManifestV2 {
            schema_version: 2,
            id: "com.kosmos.worker".into(),
            name: "Worker".into(),
            description: None,
            version: "1.0.0".into(),
            kind: PackageKind::Source,
            engine_api: ">=1.0.0".into(),
            entrypoint: "worker.exe".into(),
            icon: None,
            publisher: "kosmos".into(),
            permissions: vec![],
            targets: vec![crate::package_manifest::ManifestTarget {
                runtime: crate::package_manifest::TargetRuntime::Worker,
                os: vec![crate::package_manifest::TargetOs::Windows],
                arch: None,
            }],
            data: crate::package_manifest::ManifestData {
                access: vec![],
                defines: vec![],
                mappings: vec![],
            },
        };
        let pe = minimal_pe();
        let file = std::fs::File::create(&archive).expect("archive");
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        zip.start_file("manifest.json", options)
            .expect("manifest entry");
        zip.write_all(&serde_json::to_vec(&manifest).expect("manifest json"))
            .expect("manifest bytes");
        zip.start_file("worker.exe", options).expect("worker entry");
        zip.write_all(&pe).expect("worker bytes");
        zip.finish().expect("archive finish");

        let bytes = std::fs::read(&archive).expect("archive bytes");
        let mut digest = Sha256::new();
        digest.update(&bytes);
        let hash = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let store =
            crate::package_store::PackageStore::new(directory.path().join("store")).expect("store");
        let installed = store
            .install_versioned(
                &archive,
                bytes.len() as u64,
                &hash,
                &crate::package_manifest::VersionedManifest::V2(manifest),
                1,
            )
            .expect("install");
        let entrypoint = store
            .immutable_entrypoint(&installed)
            .expect("immutable entrypoint");
        std::fs::write(&entrypoint, b"tampered").expect("tamper");
        assert!(matches!(
            validate_executable(&entrypoint),
            Err(WorkerProcessError::InvalidExecutable)
        ));
    }
