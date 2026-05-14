// kepler-watcher — мини-watchdog для kepler.exe.
//
// Цель: запустить рядом с собой kepler.exe, держать живым, при crash'е respawn'ить
// с exp backoff (1s → 30s cap, см. план Decision #10).
//
// Сам watcher тоже в HKCU Run (через installer), это значит:
//   user login → watcher.exe стартует → spawn kepler.exe child
//   kepler.exe crashes → watcher wait + respawn
//   user logout → Windows shutdown'ит watcher → его Drop kill'ит kepler child
//
// Сейчас обычный std::process::Command — без tokio, ~50 строк.

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const MIN_BACKOFF_MS: u64 = 1_000;
const MAX_BACKOFF_MS: u64 = 30_000;
const HEALTHY_RUNTIME_MS: u64 = 60_000; // если процесс жил больше — сбрасываем backoff

fn resolve_kepler_exe() -> PathBuf {
    if let Ok(p) = env::var("KEPLER_EXE_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return path;
        }
    }
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join("kepler.exe");
            if candidate.exists() {
                return candidate;
            }
            let unix = parent.join("kepler");
            if unix.exists() {
                return unix;
            }
        }
    }
    eprintln!("[watcher] FATAL: cannot find kepler.exe near watcher binary");
    std::process::exit(2);
}

fn main() {
    let exe = resolve_kepler_exe();
    eprintln!("[watcher] starting; supervising {exe:?}");

    let mut backoff_ms: u64 = MIN_BACKOFF_MS;

    loop {
        let started = Instant::now();
        let result = Command::new(&exe)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();

        let mut child = match result {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[watcher] spawn failed: {e}; retry in {}ms",
                    backoff_ms
                );
                sleep(Duration::from_millis(backoff_ms));
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                continue;
            }
        };

        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[watcher] wait failed: {e}");
                let _ = child.kill();
                sleep(Duration::from_millis(backoff_ms));
                backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                continue;
            }
        };

        let uptime_ms = started.elapsed().as_millis() as u64;
        eprintln!(
            "[watcher] kepler exited (status: {status}, uptime: {uptime_ms}ms)",
        );

        // Если процесс прожил долго — это была "нормальная" работа, backoff обнуляем.
        // Если упал быстро — наращиваем backoff.
        if uptime_ms >= HEALTHY_RUNTIME_MS {
            backoff_ms = MIN_BACKOFF_MS;
        } else {
            backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
        }

        // Если exit status — 0 (graceful), всё равно перезапускаем, потому что
        // user мог только закрыть окно tray (Phase 6) но не убить watcher.
        // Watcher завершается только когда сам получает SIGTERM/Ctrl+C от OS.
        eprintln!(
            "[watcher] restart in {}ms (next backoff cap: {}ms)",
            backoff_ms, MAX_BACKOFF_MS
        );
        sleep(Duration::from_millis(backoff_ms));
    }
}
