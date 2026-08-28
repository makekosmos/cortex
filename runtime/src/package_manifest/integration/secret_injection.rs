use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_HEADER_NAME: usize = 64;
const MAX_AFFIX: usize = 128;
const MAX_FIXED_HEADERS: usize = 8;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationRequestMethod {
    #[default]
    Get,
    PostJson,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CookieHeaderInjection {
    pub cookie: String,
    pub header: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SecretInjection {
    Header {
        origins: Vec<String>,
        name: String,
        #[serde(default)]
        prefix: String,
    },
    Basic {
        origins: Vec<String>,
        #[serde(default)]
        password: String,
    },
    Cookies {
        origins: Vec<String>,
        #[serde(default)]
        method: IntegrationRequestMethod,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        header_from_cookie: Option<CookieHeaderInjection>,
    },
}

impl SecretInjection {
    pub(super) fn validate(&self) -> Result<(), ()> {
        match self {
            Self::Header {
                origins,
                name,
                prefix,
            } => {
                let valid_name = !name.is_empty()
                    && name.len() <= MAX_HEADER_NAME
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                    && !matches!(
                        name.to_ascii_lowercase().as_str(),
                        "cookie" | "host" | "content-length" | "connection" | "transfer-encoding"
                    );
                (valid_origins(origins) && valid_name && valid_affix(prefix))
                    .then_some(())
                    .ok_or(())
            }
            Self::Basic { origins, password } => (valid_origins(origins) && valid_affix(password))
                .then_some(())
                .ok_or(()),
            Self::Cookies {
                origins,
                headers,
                header_from_cookie,
                ..
            } => (valid_origins(origins)
                && headers.len() <= MAX_FIXED_HEADERS
                && headers
                    .iter()
                    .all(|(name, value)| valid_header(name, value))
                && header_from_cookie.as_ref().is_none_or(|mirror| {
                    valid_cookie_name(&mirror.cookie) && valid_header(&mirror.header, "placeholder")
                }))
            .then_some(())
            .ok_or(()),
        }
    }

    pub(crate) fn origins(&self) -> &[String] {
        match self {
            Self::Header { origins, .. }
            | Self::Basic { origins, .. }
            | Self::Cookies { origins, .. } => origins,
        }
    }

    pub(crate) fn request_method(&self) -> IntegrationRequestMethod {
        match self {
            Self::Cookies { method, .. } => *method,
            _ => IntegrationRequestMethod::Get,
        }
    }

    pub(crate) fn fixed_headers(&self) -> Option<&BTreeMap<String, String>> {
        match self {
            Self::Cookies { headers, .. } => Some(headers),
            _ => None,
        }
    }

    pub(crate) fn cookie_header(&self) -> Option<&CookieHeaderInjection> {
        match self {
            Self::Cookies {
                header_from_cookie, ..
            } => header_from_cookie.as_ref(),
            _ => None,
        }
    }
}

fn valid_header(name: &str, value: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "host"
            | "content-length"
            | "connection"
            | "transfer-encoding"
            | "te"
            | "trailer"
            | "upgrade"
    ) && reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_ok()
        && value.len() <= 1024
        && reqwest::header::HeaderValue::from_str(value).is_ok()
}

fn valid_cookie_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b';' | b'=' | b' '))
}

fn valid_affix(value: &str) -> bool {
    value.len() <= MAX_AFFIX && !value.contains(['\r', '\n'])
}

fn valid_origins(origins: &[String]) -> bool {
    !origins.is_empty()
        && origins.len() <= 8
        && origins.iter().all(|origin| {
            Url::parse(origin).is_ok_and(|url| {
                url.scheme() == "https"
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.path() == "/"
                    && url.query().is_none()
                    && url.fragment().is_none()
            })
        })
}
