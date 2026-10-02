// KOS-236: the Engine tray is the full replacement for the Electron shell's
// tray/command palette (Manager, Agenda, Memoria, Открыть/Выход) on
// Windows — see `windows_impl` for the menu contract.
//
// Non-Windows stays a no-op below: this Engine build only ships the tray on
// Windows, and a real Linux tray needs a StatusNotifierItem/D-Bus stack
// (e.g. `ksni`) that (a) has no `deny.toml` in this repo to check it
// against, and (b) can't be exercised on this Windows-only dev machine —
// Linux desktop tray support is also fragmented (vanilla GNOME needs an
// extension). Shipping it unverified would be its own tech debt, so it is
// left out rather than guessed at.
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum TrayEvent {
    Exit,
}

pub struct BackendTray {
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join: Option<std::thread::JoinHandle<()>>,
    // ponytail: one backend process owns one tray; keep the receiver alive
    // until shutdown even if the native thread exits early.
    _events: UnboundedSender<TrayEvent>,
}

pub fn start() -> (BackendTray, UnboundedReceiver<TrayEvent>) {
    let (events, receiver) = unbounded_channel();
    #[cfg(windows)]
    {
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let thread_events = events.clone();
        let join = std::thread::spawn(move || windows_impl::run(thread_events, ready_sender));
        let thread_id = ready_receiver.recv().unwrap_or_default();
        (
            BackendTray {
                thread_id,
                join: Some(join),
                _events: events,
            },
            receiver,
        )
    }
    #[cfg(not(windows))]
    (BackendTray { _events: events }, receiver)
}

/// Manager exe path for the `/v1/rpc` caller-identity check: identical to
/// the launch resolution in debug builds, but release builds ignore the
/// `MUNDUS_MANAGER_EXECUTABLE` env override — an env var must not decide who
/// may trigger the elevated install (KOS-269 round 4).
#[cfg(windows)]
pub(crate) fn manager_executable_for_auth() -> Option<std::path::PathBuf> {
    windows_impl::manager_executable_for_auth()
}

impl BackendTray {
    pub fn stop(self) {
        #[cfg(windows)]
        {
            let mut this = self;
            windows_impl::stop(this.thread_id);
            if let Some(join) = this.join.take() {
                let _ = join.join();
            }
        }
    }
}

#[cfg(windows)]
mod windows_impl;
