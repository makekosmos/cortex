//! kepler-focus-svc: Windows Service для focus mode hosts file management.
//!
//! Sub-commands:
//!   install         — register Windows service (start mode = manual)
//!   uninstall       — stop + remove service
//!   start           — start the service via SCM
//!   stop            — stop the service via SCM
//!   status          — print {installed, running}
//!   run-as-service  — internal entry point вызываемый SCM
//!
//! При запуске без аргументов / с неизвестным аргументом — печатает usage.
//!
//! Все SCM операции (install/uninstall/start/stop) требуют admin. Если caller
//! не admin — возвращаем `{"ok":false,"error":"...","needs_elevation":true}`
//! чтобы GUI мог перезапуститься elevated.

#[cfg(windows)]
mod cli;
#[cfg(windows)]
mod pipe;
#[cfg(windows)]
mod service;

use serde::Serialize;

#[derive(Serialize)]
struct CliResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    needs_elevation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    running: Option<bool>,
}

impl CliResponse {
    fn print_and_exit(&self) -> ! {
        let s = serde_json::to_string(self)
            .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
        println!("{s}");
        std::process::exit(if self.ok { 0 } else { 1 });
    }
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");

    match cmd {
        "install" => cli::install(),
        "uninstall" => cli::uninstall(),
        "start" => cli::start(),
        "stop" => cli::stop(),
        "status" => cli::status(),
        "run-as-service" => service::run_as_service_entry(),
        "" | "help" | "--help" | "-h" => {
            println!("usage: kepler-focus-svc <install|uninstall|start|stop|status>");
            std::process::exit(0);
        }
        other => {
            let resp = CliResponse {
                ok: false,
                error: Some(format!("unknown command: {other}")),
                needs_elevation: None,
                service_name: None,
                installed: None,
                running: None,
            };
            resp.print_and_exit();
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("kepler-focus-svc is Windows-only");
    std::process::exit(1);
}
