use super::*;

impl FileIndex {
    pub(super) async fn rescan_locked(&self) -> Result<ScanStats> {
        let scan_started = std::time::Instant::now();
        self.scan_in_progress.store(true, Ordering::SeqCst);
        self.set_progress(ScanProgressSnapshot {
            phase: "scanning".to_string(),
            roots_total: self.root_paths()?.len(),
            message: "Сканируем файлы".to_string(),
            ..Default::default()
        });
        let _progress = ScanProgressGuard {
            flag: &self.scan_in_progress,
            progress: self.scan_progress.clone(),
        };
        let generation = self.scan_generation.load(Ordering::SeqCst);
        let roots = self.root_paths()?;
        let options = self.scan_options()?;
        let progress = self.scan_progress.clone();
        let ntfs_state = self.ntfs_last_state.clone();
        let scan_generation = self.scan_generation.clone();
        let store = self.store.clone();
        let outcome = tokio::task::spawn_blocking(move || -> Result<ScanCommitOutcome> {
            // Тяжёлый WalkDir/NTFS scan + commit — на background-priority потоке
            // (CPU + I/O), чтобы не душить систему. Guard живёт внутри sync
            // closure, без `.await`. См. crate::priority.
            let _bg = crate::priority::BackgroundThreadGuard::enter();
            let progress_for_scan = progress.clone();
            let files = scanner::scan_roots_with_progress(
                &roots,
                &options,
                move |snapshot| {
                    let mut guard = progress_for_scan.lock().unwrap_or_else(|e| e.into_inner());
                    *guard = ScanProgressSnapshot {
                        phase: snapshot.phase,
                        root: snapshot.root,
                        roots_done: snapshot.roots_done,
                        roots_total: snapshot.roots_total,
                        files_seen: snapshot.files_seen,
                        files_indexed: snapshot.files_indexed,
                        message: snapshot.message,
                    };
                },
                move |status, note| {
                    let mut state = ntfs_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.status = status;
                    state.note = note;
                },
                || scan_generation.load(Ordering::SeqCst) != generation,
            );
            if scan_generation.load(Ordering::SeqCst) != generation {
                return Ok(ScanCommitOutcome::Cancelled);
            }
            {
                let mut guard = progress.lock().unwrap_or_else(|e| e.into_inner());
                *guard = ScanProgressSnapshot {
                    phase: "writing".to_string(),
                    roots_done: roots.len(),
                    roots_total: roots.len(),
                    files_seen: files.len(),
                    files_indexed: files.len(),
                    message: "Записываем индекс".to_string(),
                    ..Default::default()
                };
            }
            let total = files.len();
            store.replace_all(&files)?;
            store.checkpoint_truncate_wal()?;
            Ok(ScanCommitOutcome::Committed {
                total,
                roots: roots.len(),
                options,
            })
        })
        .await
        .map_err(|e| FileIndexError::BlockingTask(e.to_string()))??;
        let (total, roots, options) = match outcome {
            ScanCommitOutcome::Cancelled => {
                tracing::info!(target: "file_index", "rescan discarded because settings changed");
                return self.current_stats();
            }
            ScanCommitOutcome::Committed {
                total,
                roots,
                options,
            } => (total, roots, options),
        };
        // Regression C3 (2026-05-24): mutation may bump scan_generation between
        // the pre-write check and replace_all; this rescan then commits stale
        // results. Detect and trigger a follow-up rescan so eventual state is
        // correct without waiting for an external trigger.
        if self.scan_generation.load(Ordering::SeqCst) != generation {
            tracing::info!(
                target: "file_index",
                "rescan committed stale snapshot; scheduling follow-up"
            );
            self.spawn_rescan();
        }
        self.set_progress(ScanProgressSnapshot {
            phase: "done".to_string(),
            roots_done: roots,
            roots_total: roots,
            files_seen: total,
            files_indexed: total,
            message: "Индексация завершена".to_string(),
            ..Default::default()
        });
        self.last_scan_ms.store(
            scan_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            Ordering::SeqCst,
        );
        self.remember_last_scan(LastScanSnapshot {
            finished_at_unix_ms: now_unix_ms(),
            duration_ms: self.last_scan_ms.load(Ordering::SeqCst),
            indexed_file_count: total,
            roots_count: roots,
            exclude_noisy_folders: options.exclude_noisy_folders,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
        });
        Ok(ScanStats {
            enabled: true,
            total,
            roots,
            exclude_noisy_folders: options.exclude_noisy_folders,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
        })
    }
}

enum ScanCommitOutcome {
    Cancelled,
    Committed {
        total: usize,
        roots: usize,
        options: ScanOptions,
    },
}

pub fn env_flag_enabled(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
    }
}

struct ScanProgressGuard<'a> {
    flag: &'a AtomicBool,
    progress: Arc<StdMutex<ScanProgressSnapshot>>,
}

impl Drop for ScanProgressGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
        let mut progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        if progress.phase != "done" {
            progress.phase = "idle".to_string();
            progress.message = "Индексация остановлена".to_string();
        }
    }
}
