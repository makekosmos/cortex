// the Engine tray is the full replacement for the Electron shell's
// tray/command palette (Manager, Agenda, Memoria, Открыть/Выход) on
// Windows — see `windows_impl` for the menu contract.
//
// macOS uses a bundled AppKit helper process so the Engine can own the status
// item without running an AppKit event loop inside Tokio. Linux remains a
// no-op: a real tray needs a StatusNotifierItem/D-Bus backend and desktop
// environment coverage.
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum TrayEvent {
    OpenManager,
    Exit,
}

pub struct BackendTray {
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join: Option<std::thread::JoinHandle<()>>,
    #[cfg(target_os = "macos")]
    helper: Option<std::process::Child>,
    #[cfg(target_os = "macos")]
    helper_reader: Option<std::thread::JoinHandle<()>>,
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
    #[cfg(target_os = "macos")]
    {
        let (helper, helper_reader) = macos_impl::start(events.clone());
        (
            BackendTray {
                helper,
                helper_reader,
                _events: events,
            },
            receiver,
        )
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    (BackendTray { _events: events }, receiver)
}

/// Manager exe path for the `/v1/rpc` caller-identity check: identical to
/// the launch resolution in debug builds, but release builds ignore the
/// `MUNDUS_MANAGER_EXECUTABLE` env override — an env var must not decide who
/// may trigger the elevated install (round 4).
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
        #[cfg(target_os = "macos")]
        {
            let mut this = self;
            if let Some(mut helper) = this.helper.take() {
                let _ = helper.kill();
                let _ = helper.wait();
            }
            if let Some(reader) = this.helper_reader.take() {
                let _ = reader.join();
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::TrayEvent;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use tokio::sync::mpsc::UnboundedSender;

    pub(super) fn start(
        events: UnboundedSender<TrayEvent>,
    ) -> (
        Option<std::process::Child>,
        Option<std::thread::JoinHandle<()>>,
    ) {
        let Ok(engine) = std::env::current_exe() else {
            eprintln!(
                "[mundus-engine] cannot locate menu bar helper: current executable unavailable"
            );
            return (None, None);
        };
        let Some(macos_dir) = engine.parent() else {
            return (None, None);
        };
        let helper = macos_dir.join("native/macos/engine-menu-bar");
        let app = macos_dir.parent().and_then(|contents| contents.parent());
        let Some(app) = app.filter(|path| path.extension().is_some_and(|ext| ext == "app")) else {
            eprintln!("[mundus-engine] menu bar is only available inside Mundus Manager.app");
            return (None, None);
        };
        if !helper.is_file() {
            eprintln!(
                "[mundus-engine] menu bar helper missing: {}",
                helper.display()
            );
            return (None, None);
        }
        // The mark the menu bar draws, not the app icon: a template image is
        // painted in the menu bar's own colour, so it reads in light and dark.
        let icon = app.join("Contents/Resources/tray-template.png");
        let mut command = Command::new(&helper);
        command
            .arg(app)
            .arg(icon)
            .env("MUNDUS_PARENT_PID", std::process::id().to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                eprintln!("[mundus-engine] failed to start menu bar helper: {error}");
                return (None, None);
            }
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            return (None, None);
        };
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line.as_deref() {
                    Ok("open") => {
                        let _ = events.send(TrayEvent::OpenManager);
                    }
                    Ok("exit") => {
                        let _ = events.send(TrayEvent::Exit);
                    }
                    Ok(_) => eprintln!("[mundus-engine] ignored unknown menu bar command"),
                    Err(error) => {
                        eprintln!("[mundus-engine] menu bar helper read failed: {error}");
                        break;
                    }
                }
            }
        });
        (Some(child), Some(reader))
    }

    pub(super) fn open_manager() {
        let Ok(engine) = std::env::current_exe() else {
            return;
        };
        let Some(app) = engine
            .parent()
            .and_then(|macos| macos.parent())
            .and_then(|contents| contents.parent())
        else {
            return;
        };
        if let Err(error) = Command::new("/usr/bin/open").arg("-a").arg(app).spawn() {
            eprintln!("[mundus-engine] failed to open Manager: {error}");
        }
    }
}

#[cfg(target_os = "macos")]
pub fn open_manager() {
    macos_impl::open_manager();
}

#[cfg(not(target_os = "macos"))]
pub fn open_manager() {}

#[cfg(windows)]
mod windows_impl;
