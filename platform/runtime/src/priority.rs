//! Thread-scoped background priority для non-interactive maintenance работы
//! (app_index rescan, icon extraction, file_index scan, db_backup).
//!
//! На Windows `THREAD_MODE_BACKGROUND_BEGIN` роняет CPU **и** I/O приоритет
//! текущего потока, чтобы тяжёлый фон не вытеснял foreground activity и не
//! душил систему (вплоть до подвисания USB при насыщении диска). RAII: Drop
//! возвращает приоритет через `THREAD_MODE_BACKGROUND_END`.
//!
//! Жёсткие инварианты (см. `.agent/tasks/2026-06-08-background-maintenance-priority`):
//! - ❌ НЕ `PROCESS_MODE_BACKGROUND_BEGIN` на kepler-backend / ark-core-rpc —
//!   это interactive процессы (IPC/WS/search/DB), нельзя ронять весь процесс.
//! - ❌ НЕ входить в background mode внутри `async fn` через `.await`: task
//!   может resume на другом потоке pool'а → priority inversion / потерянный
//!   END. Guard живёт только внутри чистого sync closure / dedicated thread.
//! - IPC / WS / launcher search / user-triggered launch — НИКОГДА не background.

/// RAII guard: на время жизни понижает приоритет текущего потока до
/// background (CPU + I/O). На non-Windows — no-op.
#[cfg(target_os = "windows")]
pub struct BackgroundThreadGuard {
    active: bool,
}

#[cfg(target_os = "windows")]
impl BackgroundThreadGuard {
    /// Войти в thread background mode. Если syscall не удался — guard неактивен
    /// (Drop не делает END), работа просто идёт на обычном приоритете.
    pub fn enter() -> Self {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
        };
        // SAFETY: GetCurrentThread() возвращает псевдо-handle текущего потока,
        // валидный без закрытия; SetThreadPriority с background-begin меняет
        // приоритет только этого потока.
        let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
        Self { active: ok.is_ok() }
    }
}

#[cfg(target_os = "windows")]
impl Drop for BackgroundThreadGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END,
        };
        // SAFETY: симметричный END для ранее успешного BEGIN на том же потоке.
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub struct BackgroundThreadGuard;

#[cfg(not(target_os = "windows"))]
impl BackgroundThreadGuard {
    pub fn enter() -> Self {
        Self
    }
}

/// Запустить блокирующую `f` на blocking-pool потоке с понижённым background
/// приоритетом. Guard живёт ВНУТРИ sync closure (никаких `.await` внутри `f`),
/// поэтому background mode гарантированно снимается на том же потоке.
pub async fn spawn_background_blocking<F, T>(f: F) -> Result<T, tokio::task::JoinError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _bg = BackgroundThreadGuard::enter();
        f()
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_enter_and_drop_do_not_panic() {
        {
            let _g = BackgroundThreadGuard::enter();
            // тривиальная работа под guard
            let _ = (0..1000).sum::<u64>();
        }
        // Drop отработал без паники — повторный enter тоже ок.
        let _g = BackgroundThreadGuard::enter();
    }

    #[tokio::test]
    async fn spawn_background_blocking_runs_and_returns() {
        let out = spawn_background_blocking(|| 21u32 * 2).await.unwrap();
        assert_eq!(out, 42);
    }
}
