//! kepler-focus-helper: elevated short-lived process. Читает один JSON
//! request — со stdin (если запущен из админ-родителя с pipe) либо из
//! файла, переданного `--input <path>` (когда родитель запустил helper
//! через ShellExecuteEx/Start-Process `runas` verb — stdin недоступен).
//! Выполняет op над `C:\Windows\System32\drivers\etc\hosts`, печатает
//! один JSON response на stdout (либо в `--output <path>` если указан).

use std::fs;
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

struct CliArgs {
    input_file: Option<PathBuf>,
    output_file: Option<PathBuf>,
}

fn parse_cli_args() -> CliArgs {
    let mut input_file = None;
    let mut output_file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input_file = args.next().map(PathBuf::from),
            "--output" => output_file = args.next().map(PathBuf::from),
            _ => {}
        }
    }
    CliArgs { input_file, output_file }
}

fn read_request(args: &CliArgs) -> Result<String, String> {
    if let Some(p) = &args.input_file {
        fs::read_to_string(p).map_err(|e| format!("input file read failed: {e}"))
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin read failed: {e}"))?;
        Ok(buf)
    }
}

fn run(args: &CliArgs) -> Response {
    let buf = match read_request(args) {
        Ok(b) => b,
        Err(e) => return Response::err(e),
    };
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
    let args = parse_cli_args();
    let resp = run(&args);
    let json = serde_json::to_string(&resp).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"error":"serialize failed: {e}"}}"#)
    });

    // Write response — либо в --output file, либо stdout.
    if let Some(out_path) = &args.output_file {
        let _ = fs::write(out_path, &json);
    } else {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let _ = writeln!(out, "{json}");
    }

    let exit = if resp.ok { 0 } else { 1 };
    std::process::exit(exit);
}
