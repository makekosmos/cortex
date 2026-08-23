use super::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HelloMessage {
    #[serde(default)]
    pub kind: Option<String>, // ожидаем "hello"
    #[serde(rename = "protocolVersion", skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    #[serde(rename = "apiVersion", default)]
    pub api_version: Option<String>,
    pub token: Option<String>,
    pub pid: Option<u32>,
    #[serde(rename = "clientId", default)]
    pub client_id: Option<String>,
    #[serde(rename = "clientClass", default)]
    pub client_class: Option<String>,
    #[serde(rename = "clientVersion", default)]
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloOkResponse<'a> {
    pub kind: &'static str, // "hello_ok"
    #[serde(rename = "apiVersion")]
    pub api_version: &'a str,
    pub compatibility: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloErrorResponse<'a> {
    pub kind: &'static str, // "hello_error"
    pub code: &'a str,
    pub message: String,
}

/// Результат валидации hello — что отправить клиенту перед основным циклом.
#[derive(Debug, Clone)]
pub enum HelloOutcome {
    Accept {
        compatibility: Compatibility,
        transport: TransportKind,
    },
    Reject {
        code: &'static str,
        message: String,
    },
}

/// Чистая функция (детерминированная) — отделена от network IO, тестируется легко.
pub fn validate_hello(hello: &HelloMessage, expected_token: &str) -> HelloOutcome {
    if hello.protocol_version.is_some() {
        return HelloOutcome::Reject {
            code: handshake_errors::UPGRADE_REQUIRED,
            message: "protocolVersion is no longer supported; upgrade to apiVersion".into(),
        };
    }
    let Some(raw_version) = hello.api_version.as_deref() else {
        return HelloOutcome::Reject {
            code: handshake_errors::MISSING_PROTOCOL_VERSION,
            message: "client must send apiVersion in hello".into(),
        };
    };
    if hello.kind.as_deref() != Some("hello") {
        return HelloOutcome::Reject {
            code: handshake_errors::MALFORMED_HELLO,
            message: "first frame kind must be hello".into(),
        };
    }

    let parsed_version = match ProtocolVersion::parse(raw_version) {
        Ok(v) => v,
        Err(e) => {
            return HelloOutcome::Reject {
                code: handshake_errors::MALFORMED_PROTOCOL_VERSION,
                message: format!("invalid apiVersion {raw_version:?}: {e}"),
            };
        }
    };

    let server_version = &API_VERSION_CURRENT;
    let compatibility = parsed_version.is_compatible_with_server(server_version);
    if matches!(compatibility, Compatibility::Incompatible) {
        return HelloOutcome::Reject {
            code: handshake_errors::INCOMPATIBLE_PROTOCOL_VERSION,
            message: format!(
                "client protocol MAJOR={} differs from server MAJOR={}",
                parsed_version.major, server_version.major
            ),
        };
    }

    let token = match &hello.token {
        Some(t) => t,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_TOKEN,
                message: "client must send auth token in hello".into(),
            };
        }
    };

    if !auth::validate_token(token, expected_token) {
        return HelloOutcome::Reject {
            code: handshake_errors::INVALID_TOKEN,
            message: "auth token does not match server's lock-file token".into(),
        };
    }

    let pid = match hello.pid {
        Some(p) => p,
        None => {
            return HelloOutcome::Reject {
                code: handshake_errors::MISSING_PID,
                message: "client must send its OS PID for PID-binding".into(),
            };
        }
    };

    match auth::validate_pid_belongs_to_current_user(pid) {
        Ok(()) => HelloOutcome::Accept {
            compatibility,
            transport: TransportKind::ApiV1,
        },
        Err(auth::AuthError::PidNotFound { .. }) => HelloOutcome::Reject {
            code: handshake_errors::INVALID_PID,
            message: "PID authorization failed".into(),
        },
        Err(auth::AuthError::ForeignUserPid { .. }) => HelloOutcome::Reject {
            code: handshake_errors::FOREIGN_USER_PID,
            message: format!("PID {pid} belongs to another user account"),
        },
        Err(other) => HelloOutcome::Reject {
            code: handshake_errors::INVALID_PID,
            message: format!("PID-binding check failed: {other}"),
        },
    }
}

pub fn compatibility_label(c: &Compatibility) -> &'static str {
    match c {
        Compatibility::Exact => "exact",
        Compatibility::MinorMismatch => "minor_mismatch",
        Compatibility::Incompatible => "incompatible",
    }
}
