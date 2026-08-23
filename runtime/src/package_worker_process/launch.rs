#[cfg(windows)]
async fn launch_suspended(
    executable: &Path,
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> Result<WorkerProcess, WorkerProcessError> {
    owner.claim()?;
    let pipe_name = |suffix: &str| {
        format!(
            r"\\.\pipe\kosmos-worker-{}-{}-{}",
            std::process::id(),
            PIPE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            suffix
        )
    };

    #[cfg(windows)]
    fn create_named_pipe(
        name: &str,
        parent_reads: bool,
    ) -> Result<(WorkerPipe, OwnedHandle), WorkerProcessError> {
        use std::os::windows::io::AsRawHandle;
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::Security::SECURITY_ATTRIBUTES;
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: true.into(),
        };
        let mut options = ServerOptions::new();
        options
            .access_inbound(parent_reads)
            .access_outbound(!parent_reads);
        let server = options
            .create(name)
            .map_err(|_| WorkerProcessError::Setup)?;
        let mut client_options = ClientOptions::new();
        let (child_reads, child_writes) = child_pipe_access(parent_reads);
        client_options.read(child_reads).write(child_writes);
        let client = unsafe {
            client_options
                .open_with_security_attributes_raw(
                    name,
                    &mut sa as *mut SECURITY_ATTRIBUTES as *mut std::ffi::c_void,
                )
                .map_err(|_| WorkerProcessError::Setup)?
        };
        let raw = client.as_raw_handle();
        std::mem::forget(client);
        Ok((server, OwnedHandle(HANDLE(raw))))
    }

    // Parent owns the Tokio server ends; child owns the inheritable client ends.
    // The client handles are kept in RAII until CreateProcess has consumed them.
    if should_fail_stage(FailureStage::Pipe) {
        owner.abandon_claim();
        return Err(WorkerProcessError::Setup);
    }
    let (stdin, child_stdin) = match create_named_pipe(&pipe_name("stdin"), false) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let (stdout, child_stdout) = match create_named_pipe(&pipe_name("stdout"), true) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let (stderr, child_stderr) = match create_named_pipe(&pipe_name("stderr"), true) {
        Ok(value) => value,
        Err(error) => {
            owner.abandon_claim();
            return Err(error);
        }
    };
    let child_handles = [child_stdin, child_stdout, child_stderr];
    {
        let child_raw = child_handles
            .iter()
            .map(OwnedHandle::raw)
            .collect::<Vec<_>>();
        match create_process_suspended(executable, &child_raw, &owner) {
            Ok(()) => (),
            Err(error) => {
                owner.abandon_claim();
                return Err(error);
            }
        }
    }
    // This guard remains the sole owner of process/thread/job authority through
    // every await point.
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    if let Some(barrier) = take_pipe_connect_barrier() {
        let _ = barrier.ready.send(());
        let _ = barrier.release.await;
    }
    // Establish all IOCP connections while the primary thread is still suspended.
    if stdin.connect().await.is_err()
        || stdout.connect().await.is_err()
        || stderr.connect().await.is_err()
    {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    if let Err(error) = configure_and_install_job(&owner) {
        return Err(cleanup_result_async(error, owner.clone(), deadline).await);
    }
    if should_fail_stage(FailureStage::PreResume) {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    if let Some(barrier) = take_resume_barrier() {
        let _ = barrier.ready.send(());
        let _ = barrier.release.await;
    }
    if let Err(error) = owner.resume() {
        return Err(cleanup_result_async(error, owner.clone(), deadline).await);
    }
    #[cfg(all(windows, any(test, feature = "package-worker-fixture")))]
    RESUME_COUNT.fetch_add(1, Ordering::SeqCst);
    if should_fail_stage(FailureStage::Wrapper) {
        return Err(cleanup_result_async(WorkerProcessError::Setup, owner.clone(), deadline).await);
    }
    // The child client handles are no longer needed by the parent after resume.
    drop(child_handles);
    match owner.commit(stdin, stdout, stderr) {
        Ok(process) => Ok(process),
        Err(error) => Err(cleanup_result_async(error, owner, deadline).await),
    }
}

#[cfg(windows)]
static PIPE_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg(windows)]
fn cleanup_result(
    setup: WorkerProcessError,
    cleanup: Result<(), WorkerProcessError>,
) -> WorkerProcessError {
    cleanup
        .map(|_| setup)
        .unwrap_or(WorkerProcessError::Cleanup)
}

#[cfg(windows)]
async fn cleanup_result_async(
    setup: WorkerProcessError,
    owner: Arc<LaunchCleanupOwner>,
    deadline: Instant,
) -> WorkerProcessError {
    cleanup_result(setup, owner.cleanup_until(deadline).await)
}
