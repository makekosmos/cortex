//! kepler-focus-helper: elevated short-lived process. Читает один JSON
//! request с stdin, выполняет op над `C:\Windows\System32\drivers\etc\hosts`,
//! печатает один JSON response на stdout, выходит.

use std::io::{self, Read, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use kepler_focus_helper::hosts;

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Request {
    Add { domains: Vec<String> },
    Remove { domains: Vec<String> },
    Reset,
    Status,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok(domains: Vec<String>) -> Self {
        Self {
            ok: true,
            active_domains: Some(domains),
            error: None,
        }
    }

    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            active_domains: None,
            error: Some(msg.into()),
        }
    }
}

fn hosts_path() -> PathBuf {
    // Override через env переменную для интеграционных smoke тестов.
    if let Ok(p) = std::env::var("KEPLER_FOCUS_HOSTS_PATH") {
        return PathBuf::from(p);
    }
    #[cfg(windows)]
    {
        let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        PathBuf::from(sysroot).join("System32\\drivers\\etc\\hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

fn run() -> Response {
    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        return Response::err(format!("stdin read failed: {e}"));
    }
    let req: Request = match serde_json::from_str(buf.trim()) {
        Ok(r) => r,
        Err(e) => return Response::err(format!("invalid request: {e}")),
    };

    let path = hosts_path();
    let result = match req {
        Request::Add { domains } => hosts::add_domains(&path, &domains),
        Request::Remove { domains } => hosts::remove_domains(&path, &domains),
        Request::Reset => hosts::reset(&path),
        Request::Status => hosts::read_active_domains(&path),
    };

    match result {
        Ok(active) => Response::ok(active),
        Err(e) => Response::err(e.to_string()),
    }
}

fn main() {
    let resp = run();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let json = serde_json::to_string(&resp).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"error":"serialize failed: {e}"}}"#)
    });
    let _ = writeln!(out, "{json}");
    let exit = if resp.ok { 0 } else { 1 };
    std::process::exit(exit);
}
