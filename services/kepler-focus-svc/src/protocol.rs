//! Wire protocol для named pipe IPC. Request/Response типы + `dispatch`
//! функция, которая по Request вызывает `kepler_focus_helper::hosts::*` и
//! возвращает Response. Параметризовано путём к hosts file → тестируемо без
//! админских прав.

use std::path::Path;

use serde::{Deserialize, Serialize};

use kepler_focus_helper::hosts;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Request {
    Add { domains: Vec<String> },
    Remove { domains: Vec<String> },
    Reset,
    Status,
    Ping,
    #[cfg(windows)]
    #[serde(rename = "ntfs_scan")]
    NtfsScan {
        root: String,
        exclude_noisy: bool,
    },
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pong: Option<bool>,
    #[cfg(windows)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<crate::ntfs_scan::NtfsScanEntry>>,
}

impl Response {
    pub fn ok_domains(domains: Vec<String>) -> Self {
        Self {
            ok: true,
            active_domains: Some(domains),
            error: None,
            pong: None,
            #[cfg(windows)]
            files: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            active_domains: None,
            error: Some(msg.into()),
            pong: None,
            #[cfg(windows)]
            files: None,
        }
    }

    pub fn pong() -> Self {
        Self {
            ok: true,
            active_domains: None,
            error: None,
            pong: Some(true),
            #[cfg(windows)]
            files: None,
        }
    }

    #[cfg(windows)]
    pub fn files(files: Vec<crate::ntfs_scan::NtfsScanEntry>) -> Self {
        Self {
            ok: true,
            active_domains: None,
            error: None,
            pong: None,
            files: Some(files),
        }
    }
}

/// Парсит JSON request и выполняет op над hosts file по `hosts_path`.
/// Никогда не паникует — все ошибки идут как Response::err.
pub fn handle_raw(raw: &str, hosts_path: &Path) -> Response {
    let req: Request = match serde_json::from_str(raw.trim()) {
        Ok(r) => r,
        Err(e) => return Response::err(format!("invalid request: {e}")),
    };
    dispatch(req, hosts_path)
}

pub fn dispatch(req: Request, hosts_path: &Path) -> Response {
    let result = match req {
        Request::Add { domains } => hosts::add_domains(hosts_path, &domains),
        Request::Remove { domains } => hosts::remove_domains(hosts_path, &domains),
        Request::Reset => hosts::reset(hosts_path),
        Request::Status => hosts::read_active_domains(hosts_path),
        Request::Ping => return Response::pong(),
        #[cfg(windows)]
        Request::NtfsScan {
            root,
            exclude_noisy,
        } => {
            return match crate::ntfs_scan::scan_drive_root(&root, exclude_noisy) {
                Ok(files) => Response::files(files),
                Err(e) => Response::err(e),
            };
        }
    };
    match result {
        Ok(active) => Response::ok_domains(active),
        Err(e) => Response::err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const ORIGINAL: &str = "# Copyright (c) Microsoft Corp.\n127.0.0.1 localhost\n";

    fn setup() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let hosts = dir.path().join("hosts");
        fs::write(&hosts, ORIGINAL).unwrap();
        (dir, hosts)
    }

    #[test]
    fn parse_add_request() {
        let raw = r#"{"op":"add","domains":["tiktok.com"]}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(
            req,
            Request::Add {
                domains: vec!["tiktok.com".into()]
            }
        );
    }

    #[test]
    fn parse_remove_request() {
        let raw = r#"{"op":"remove","domains":["x.com","y.com"]}"#;
        let req: Request = serde_json::from_str(raw).unwrap();
        assert_eq!(
            req,
            Request::Remove {
                domains: vec!["x.com".into(), "y.com".into()]
            }
        );
    }

    #[test]
    fn parse_reset_status_ping() {
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"op":"reset"}"#).unwrap(),
            Request::Reset
        );
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"op":"status"}"#).unwrap(),
            Request::Status
        );
        assert_eq!(
            serde_json::from_str::<Request>(r#"{"op":"ping"}"#).unwrap(),
            Request::Ping
        );
    }

    #[test]
    fn invalid_json_returns_error_response() {
        let (_d, hosts) = setup();
        let resp = handle_raw("not json", &hosts);
        assert!(!resp.ok);
        assert!(resp.error.as_deref().unwrap().contains("invalid"));
    }

    #[test]
    fn unknown_op_returns_error_response() {
        let (_d, hosts) = setup();
        let resp = handle_raw(r#"{"op":"obliterate"}"#, &hosts);
        assert!(!resp.ok);
        assert!(resp.error.is_some());
    }

    #[test]
    fn ping_returns_pong_response() {
        let (_d, hosts) = setup();
        let resp = handle_raw(r#"{"op":"ping"}"#, &hosts);
        assert!(resp.ok);
        assert_eq!(resp.pong, Some(true));
        assert!(resp.active_domains.is_none());
    }

    #[test]
    fn add_dispatch_writes_hosts() {
        let (_d, hosts) = setup();
        let resp = handle_raw(r#"{"op":"add","domains":["tiktok.com"]}"#, &hosts);
        assert!(resp.ok, "expected ok, got {:?}", resp.error);
        let active = resp.active_domains.unwrap();
        assert_eq!(active, vec!["tiktok.com".to_string()]);
        let content = fs::read_to_string(&hosts).unwrap();
        assert!(content.contains("127.0.0.1 tiktok.com"));
    }

    #[test]
    fn status_returns_active_domains() {
        let (_d, hosts) = setup();
        handle_raw(r#"{"op":"add","domains":["a.com","b.com"]}"#, &hosts);
        let resp = handle_raw(r#"{"op":"status"}"#, &hosts);
        assert!(resp.ok);
        let mut active = resp.active_domains.unwrap();
        active.sort();
        assert_eq!(active, vec!["a.com".to_string(), "b.com".to_string()]);
    }

    #[test]
    fn reset_clears_managed_section() {
        let (_d, hosts) = setup();
        handle_raw(r#"{"op":"add","domains":["x.com"]}"#, &hosts);
        let resp = handle_raw(r#"{"op":"reset"}"#, &hosts);
        assert!(resp.ok);
        assert!(resp.active_domains.unwrap().is_empty());
    }

    #[test]
    fn response_serializes_without_none_fields() {
        let resp = Response::ok_domains(vec!["a.com".into()]);
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("active_domains"));
        assert!(!s.contains("error"));
        assert!(!s.contains("pong"));
    }

    #[test]
    fn err_response_shape() {
        let resp = Response::err("boom");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":false"));
        assert!(s.contains("\"error\":\"boom\""));
        assert!(!s.contains("active_domains"));
    }

    #[test]
    fn pong_response_shape() {
        let resp = Response::pong();
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"ok\":true"));
        assert!(s.contains("\"pong\":true"));
    }
}
