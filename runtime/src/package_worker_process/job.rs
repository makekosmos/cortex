#[cfg(windows)]
fn configure_and_install_job(owner: &LaunchCleanupOwner) -> Result<(), WorkerProcessError> {
    use std::mem::size_of;
    use windows::Win32::System::JobObjects::*;
    let mut state = owner.lock();
    if state.state != OwnerState::Created || state.process.is_none() || state.job.is_some() {
        return Err(WorkerProcessError::Setup);
    }
    let process = state
        .process
        .as_ref()
        .ok_or(WorkerProcessError::Setup)?
        .handle
        .raw();
    if should_fail_stage(FailureStage::JobCreate) {
        return Err(WorkerProcessError::Setup);
    }
    let job = OwnedHandle(unsafe {
        CreateJobObjectW(None, None).map_err(|_| WorkerProcessError::Setup)?
    });
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_JOB_MEMORY;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.JobMemoryLimit = 512 * 1024 * 1024;
    if should_fail_stage(FailureStage::Limits)
        || unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut _) as *const _,
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .is_err()
        }
    {
        return Err(WorkerProcessError::Setup);
    }
    let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
        ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2500 },
    };
    if should_fail_stage(FailureStage::Cpu)
        || unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectCpuRateControlInformation,
                (&mut cpu as *mut _) as *const _,
                size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
            )
            .is_err()
        }
    {
        return Err(WorkerProcessError::Setup);
    }
    if should_fail_stage(FailureStage::Assign)
        || unsafe { AssignProcessToJobObject(job.raw(), process).is_err() }
    {
        return Err(WorkerProcessError::Setup);
    }
    state.job = Some(JobHandle(job));
    state.state = OwnerState::JobInstalled;
    Ok(())
}

#[cfg(windows)]
fn duplicate_handle(handle: &OwnedHandle) -> Result<OwnedHandle, WorkerProcessError> {
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut copy = windows::Win32::Foundation::HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle.raw(),
            GetCurrentProcess(),
            &mut copy,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(|_| WorkerProcessError::Setup)?;
    }
    Ok(OwnedHandle(copy))
}
