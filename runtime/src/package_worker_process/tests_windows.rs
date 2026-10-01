    #[cfg(windows)]
    #[tokio::test]
    async fn reused_owner_is_rejected_before_create_process() {
        let _test_lock = windows_worker_test_lock();
        let owner = LaunchCleanupOwner::new();
        owner.claim().expect("initial claim");
        test_support::capture_next_process();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let result = WorkerProcess::launch_with_owner(executable, owner.clone()).await;
        assert!(matches!(result, Err(WorkerProcessError::Setup)));
        assert!(test_support::take_captured_process().is_none());
        assert!(owner.is_armed());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn concurrent_same_owner_launches_reject_before_second_create_process() {
        let _test_lock = windows_worker_test_lock();
        let owner = LaunchCleanupOwner::new();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let mut barrier = test_support::pause_next_before_resume();
        test_support::reset();
        let first = tokio::spawn(WorkerProcess::launch_with_owner(
            executable.clone(),
            owner.clone(),
        ));
        barrier.suspended_ready().await;
        let second = WorkerProcess::launch_with_owner(executable, owner.clone()).await;
        assert!(matches!(second, Err(WorkerProcessError::Setup)));
        assert_eq!(test_support::created_process_count(), 1);
        barrier.release();
        let mut first = first
            .await
            .expect("first launch task")
            .expect("first launch");
        first.stop().await.expect("stop first launch");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn committed_owner_reuse_is_rejected_without_create_process() {
        let _test_lock = windows_worker_test_lock();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        let owner = LaunchCleanupOwner::new();
        test_support::reset();
        let mut process = WorkerProcess::launch_with_owner(executable.clone(), owner.clone())
            .await
            .expect("launch");
        let created = test_support::created_process_count();
        let result = WorkerProcess::launch_with_owner(executable, owner).await;
        assert!(matches!(result, Err(WorkerProcessError::Setup)));
        assert_eq!(test_support::created_process_count(), created);
        process.stop().await.expect("stop");
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_owner_cleanup_restores_armed_state_and_retries_reap() {
        let _test_lock = windows_worker_test_lock();
        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        test_support::reset();
        let owner = LaunchCleanupOwner::new();
        test_support::capture_next_process();
        let mut gate = test_support::pause_next_cleanup_wait();
        test_support::fail_next(FailureStage::PreResume);
        let launch = tokio::spawn(WorkerProcess::launch_with_owner(
            executable.clone(),
            owner.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(5), gate.ready())
            .await
            .expect("cleanup wait gate");
        assert_eq!(owner.lock().state, OwnerState::Cleaning);
        assert!(owner.lock().process.is_some());
        assert!(owner.lock().job.is_some());
        assert!(matches!(
            owner
                .cleanup_until(Instant::now() + Duration::from_secs(10))
                .await,
            Err(WorkerProcessError::Cleanup)
        ));
        assert!(matches!(owner.claim(), Err(WorkerProcessError::Setup)));
        assert!(matches!(owner.resume(), Err(WorkerProcessError::Setup)));
        assert!(matches!(
            WorkerProcess::launch_with_owner(executable, owner.clone()).await,
            Err(WorkerProcessError::Setup)
        ));
        launch.abort();
        assert!(matches!(launch.await, Err(error) if error.is_cancelled()));
        assert_eq!(owner.lock().state, OwnerState::JobInstalled);
        assert!(owner.lock().terminated);
        assert!(owner.lock().process.is_some());
        assert!(owner.lock().job.is_some());
        gate.release();
        owner
            .cleanup_until(Instant::now() + Duration::from_secs(10))
            .await
            .expect("retry cleanup");
        {
            let state = owner.lock();
            assert_eq!(state.state, OwnerState::Clean);
            assert!(state.process.is_none());
            assert!(state.thread.is_none());
            assert!(state.job.is_none());
        }
        assert!(test_support::take_captured_process()
            .expect("captured process")
            .wait_object_0());
        assert!(test_support::take_captured_process().is_none());
    }

    #[tokio::test]
    async fn non_windows_fails_closed() {
        #[cfg(not(windows))]
        {
            let directory = tempfile::tempdir().expect("temporary directory");
            let path = directory.path().join("worker.exe");
            std::fs::write(&path, minimal_pe()).expect("temporary executable");
            assert!(matches!(
                WorkerProcess::launch(path).await,
                Err(WorkerProcessError::UnsupportedPlatform)
            ));
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn stopping_worker_does_not_kill_unrelated_process() {
        let _test_lock = windows_worker_test_lock();
        use std::process::Command as StdCommand;
        // Spawn ping directly, not via `cmd /c`: killing cmd would orphan the
        // grandchild ping.exe, which then keeps the inherited stdio pipe open
        // and fails nextest's leaked-handle check (KOS-291).
        let mut unrelated = StdCommand::new("ping")
            .args(["127.0.0.1", "-n", "5"])
            .spawn()
            .expect("unrelated child");
        let comspec = std::env::var_os("COMSPEC").expect("COMSPEC");
        let mut worker = WorkerProcess::launch_with_owner(&comspec, LaunchCleanupOwner::new())
            .await
            .expect("worker child");
        worker.stop().await.expect("stop worker");
        assert!(unrelated.try_wait().expect("poll unrelated").is_none());
        let _ = unrelated.kill();
        let _ = unrelated.wait();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn launch_reaps_child_for_each_job_setup_failure() {
        let _test_lock = windows_worker_test_lock();
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::System::Threading::WaitForSingleObject;

        let executable = std::env::var_os("COMSPEC").expect("COMSPEC");
        for stage in [
            FailureStage::JobCreate,
            FailureStage::Limits,
            FailureStage::Cpu,
            FailureStage::Assign,
            FailureStage::PreResume,
            FailureStage::Resume,
        ] {
            LAST_SPAWNED_HANDLE.store(0, Ordering::SeqCst);
            JOB_SETUP_FAILURE.store(stage as u8, Ordering::SeqCst);
            CAPTURE_CHILD_HANDLE.store(true, Ordering::SeqCst);
            let result = WorkerProcess::launch_with_owner(
                PathBuf::from(executable.clone()),
                LaunchCleanupOwner::new(),
            )
            .await;
            CAPTURE_CHILD_HANDLE.store(false, Ordering::SeqCst);
            assert!(matches!(result, Err(WorkerProcessError::Setup)));
            assert_eq!(JOB_SETUP_FAILURE.load(Ordering::SeqCst), 0);
            let raw_handle = LAST_SPAWNED_HANDLE.swap(0, Ordering::SeqCst);
            assert_ne!(raw_handle, 0, "child was not spawned for {stage:?}");
            let handle = HANDLE(raw_handle as _);
            assert_eq!(unsafe { WaitForSingleObject(handle, 2_000) }, WAIT_OBJECT_0);
            unsafe {
                CloseHandle(handle).expect("close retained child handle");
            }
        }
    }
