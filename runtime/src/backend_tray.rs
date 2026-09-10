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
        return (
            BackendTray {
                thread_id,
                join: Some(join),
                _events: events,
            },
            receiver,
        );
    }
    #[cfg(not(windows))]
    (BackendTray { _events: events }, receiver)
}

impl BackendTray {
    pub fn stop(mut self) {
        #[cfg(windows)]
        {
            windows_impl::stop(self.thread_id);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}

#[cfg(windows)]
mod windows_impl;
