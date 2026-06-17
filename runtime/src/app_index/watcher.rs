// Watcher — fs-watch для Start Menu dirs + TTL fallback.
//
// v1: только base infrastructure. Подключается из main.rs supervisor'а.
// UWP PackageCatalog watch — отдельно в platform/windows/uwp.rs.
//
// Дизайн: при событии — debounce 500ms → emit `app_index.rescan_requested`
// тому, кто владеет AppIndex (через mpsc channel или command-bus event).

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub struct AppIndexWatcher {
    _watcher: RecommendedWatcher,
    pub rx: mpsc::Receiver<()>,
}

impl AppIndexWatcher {
    /// Создать watcher для указанных директорий. Все события дебаунсятся в
    /// одно сообщение в `rx` (с задержкой ~500ms).
    pub fn new(paths: Vec<PathBuf>) -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event);
                }
            },
            Config::default(),
        )?;

        for p in &paths {
            if p.exists() {
                watcher.watch(p, RecursiveMode::Recursive)?;
            }
        }

        let (debounced_tx, debounced_rx) = mpsc::channel::<()>();
        std::thread::spawn(move || {
            let debounce = Duration::from_millis(500);
            loop {
                // Block on first event.
                let _ = match raw_rx.recv() {
                    Ok(e) => e,
                    Err(_) => return,
                };
                // Drain any further events within debounce window.
                let start = std::time::Instant::now();
                while start.elapsed() < debounce {
                    match raw_rx.recv_timeout(debounce - start.elapsed()) {
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
                if debounced_tx.send(()).is_err() {
                    return;
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            rx: debounced_rx,
        })
    }
}
