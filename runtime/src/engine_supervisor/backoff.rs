use super::*;

pub(crate) async fn wait_before_restart(
    state_path: &Path,
    crash_streak: usize,
    core_pid: Option<u32>,
    last_exit: Option<&ExitMetadata>,
) -> bool {
    let index = crash_streak.saturating_sub(1);
    let Some(delay) = RESTART_DELAYS.get(index).copied() else {
        return true;
    };
    write_state(state_path, "restarting", crash_streak, core_pid, last_exit);
    crate::observability::stderr(format!(
        "[kosmos-engine] restart attempt {}/{} in {}s",
        crash_streak,
        RESTART_DELAYS.len(),
        delay.as_secs()
    ));
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        _ = tokio::signal::ctrl_c() => false,
    }
}

pub(crate) fn next_crash_streak(current: usize, ran_for: Duration) -> usize {
    if ran_for >= SUCCESSFUL_RUN {
        1
    } else {
        current.saturating_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitKind {
    Intentional,
    Unexpected,
}

pub(crate) fn crash_streak_after_exit(current: usize, ran_for: Duration, kind: ExitKind) -> usize {
    match kind {
        ExitKind::Intentional => current,
        ExitKind::Unexpected => next_crash_streak(current, ran_for),
    }
}

pub(crate) fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}
