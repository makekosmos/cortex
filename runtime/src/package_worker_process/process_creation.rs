#[cfg(windows)]
fn create_process_suspended(
    executable: &Path,
    child_raw: &[windows::Win32::Foundation::HANDLE],
    owner: &LaunchCleanupOwner,
) -> Result<(), WorkerProcessError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };
    let mut attribute_size = 0usize;
    unsafe {
        let _ = InitializeProcThreadAttributeList(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(std::ptr::null_mut()),
            1,
            0,
            &mut attribute_size,
        );
    }
    if attribute_size == 0 {
        return Err(WorkerProcessError::Setup);
    }
    let mut attribute_storage = vec![0u8; attribute_size];
    let attribute_list = attribute_storage.as_mut_ptr() as *mut std::ffi::c_void;
    unsafe {
        InitializeProcThreadAttributeList(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list),
            1,
            0,
            &mut attribute_size,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }
    struct AttributeGuard(*mut std::ffi::c_void);
    impl Drop for AttributeGuard {
        fn drop(&mut self) {
            unsafe {
                DeleteProcThreadAttributeList(
                    windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(self.0),
                );
            }
        }
    }
    let _attribute_guard = AttributeGuard(attribute_list);
    if should_fail_stage(FailureStage::Attribute) {
        return Err(WorkerProcessError::Setup);
    }
    if child_raw.len() != 3
        || child_raw.iter().any(|handle| handle.is_invalid())
        || child_raw
            .iter()
            .enumerate()
            .any(|(index, handle)| child_raw[index + 1..].contains(handle))
    {
        return Err(WorkerProcessError::Setup);
    }
    let child_bytes =
        handle_allowlist_byte_len(child_raw.len()).ok_or(WorkerProcessError::Setup)?;
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    capture_handle_allowlist(child_raw.len(), child_bytes);
    unsafe {
        UpdateProcThreadAttribute(
            windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(child_raw.as_ptr() as *const _),
            child_bytes,
            None,
            None,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }

    let mut si = STARTUPINFOEXW::default();
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = child_raw[0];
    si.StartupInfo.hStdOutput = child_raw[1];
    si.StartupInfo.hStdError = child_raw[2];
    si.lpAttributeList =
        windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(attribute_list);
    let mut pi = PROCESS_INFORMATION::default();
    let mut environment = std::collections::BTreeMap::<String, (String, std::ffi::OsString)>::new();
    #[cfg(any(test, feature = "package-worker-fixture"))]
    let environment_names = [
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        // Fixture markers are test-only process inputs. Production launches
        // never copy these names from the parent environment.
        "KOSMOS_FIXTURE_ENTRY_MARKER",
        "KOSMOS_FIXTURE_BOOTSTRAP_MARKER",
    ];
    #[cfg(not(any(test, feature = "package-worker-fixture")))]
    let environment_names = ["SystemRoot", "WINDIR", "TEMP", "TMP"];
    for name in environment_names {
        if let Some(value) = std::env::var_os(name) {
            environment
                .entry(name.to_ascii_lowercase())
                .or_insert_with(|| (name.into(), value));
        }
    }
    let mut env = Vec::<u16>::new();
    for (_, (name, value)) in environment {
        env.extend(std::ffi::OsStr::new(&name).encode_wide());
        env.push('=' as u16);
        env.extend(value.encode_wide());
        env.push(0);
    }
    env.push(0);
    let mut app = executable.as_os_str().encode_wide().collect::<Vec<_>>();
    app.push(0);
    let mut current_dir = executable
        .parent()
        .unwrap_or_else(|| Path::new("C:\\Windows\\System32"))
        .as_os_str()
        .encode_wide()
        .collect::<Vec<_>>();
    current_dir.push(0);
    let mut owner_state = owner.lock();
    if owner_state.state != OwnerState::Claimed
        || owner_state.process.is_some()
        || owner_state.thread.is_some()
        || owner_state.job.is_some()
    {
        return Err(WorkerProcessError::Setup);
    }
    let created = !should_fail_stage(FailureStage::CreateProcess)
        && unsafe {
            CreateProcessW(
                PCWSTR(app.as_ptr()),
                windows::core::PWSTR::null(),
                None,
                None,
                true,
                windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(
                    CREATE_SUSPENDED
                        | CREATE_NO_WINDOW
                        | CREATE_UNICODE_ENVIRONMENT
                        | EXTENDED_STARTUPINFO_PRESENT,
                ),
                Some(env.as_ptr() as *const _),
                PCWSTR(current_dir.as_ptr()),
                &si as *const STARTUPINFOEXW
                    as *const windows::Win32::System::Threading::STARTUPINFOW,
                &mut pi,
            )
            .is_ok()
        };
    if !created {
        owner_state.state = OwnerState::Clean;
        return Err(WorkerProcessError::Setup);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    {
        CREATED_PROCESS_COUNT.fetch_add(1, Ordering::SeqCst);
        capture_process_handle(pi.hProcess);
    }
    owner_state.process = Some(ProcessHandle {
        handle: OwnedHandle(pi.hProcess),
        id: pi.dwProcessId,
    });
    owner_state.thread = Some(OwnedHandle(pi.hThread));
    owner_state.state = OwnerState::Created;
    Ok(())
}

#[cfg(any(test, windows))]
fn checked_handle_allowlist_byte_len(handle_count: usize, handle_size: usize) -> Option<usize> {
    (handle_count != 0)
        .then(|| handle_count.checked_mul(handle_size))
        .flatten()
}

#[cfg(windows)]
fn handle_allowlist_byte_len(handle_count: usize) -> Option<usize> {
    checked_handle_allowlist_byte_len(
        handle_count,
        size_of::<windows::Win32::Foundation::HANDLE>(),
    )
}
