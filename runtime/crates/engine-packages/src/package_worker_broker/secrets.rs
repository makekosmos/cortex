//! Broker secret injection for worker fetch: manifest-bound, never
//! echoed back to the worker.

use super::*;
use crate::package_manifest::IntegrationRequestMethod;
use std::collections::HashMap;

pub(super) fn request_body(
    method: IntegrationRequestMethod,
    body: Option<&serde_json::Value>,
) -> Result<Option<Vec<u8>>, BrokerError> {
    Ok(match (method, body) {
        (IntegrationRequestMethod::Get, None) => None,
        (IntegrationRequestMethod::PostJson, Some(body)) => {
            let bytes = serde_json::to_vec(body)
                .map_err(|_| BrokerError::Invalid("invalid JSON body".into()))?;
            if bytes.len() > MAX_JSON_BODY {
                return Err(BrokerError::Invalid("JSON body exceeds 64 KiB".into()));
            }
            Some(bytes)
        }
        _ => return Err(BrokerError::Invalid("request method/body mismatch".into())),
    })
}

fn session_field<'a>(session: &'a serde_json::Value, field: &str) -> Result<&'a str, BrokerError> {
    session
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 8192)
        .ok_or_else(|| BrokerError::Invalid("missing or invalid session field".into()))
}

pub(super) fn inject_json_body(
    body: Option<&serde_json::Value>,
    secret: Option<&SecretRequest<'_>>,
) -> Result<Option<serde_json::Value>, BrokerError> {
    let Some(SecretRequest {
        injection: SecretInjection::Json { body_fields, .. },
        secret,
        ..
    }) = secret
    else {
        return Ok(None);
    };
    let session: serde_json::Value = serde_json::from_str(secret)
        .map_err(|_| BrokerError::Invalid("invalid JSON session".into()))?;
    let mut body = body
        .and_then(serde_json::Value::as_object)
        .cloned()
        .ok_or_else(|| BrokerError::Invalid("JSON session requires an object body".into()))?;
    for (name, field) in body_fields {
        if body.contains_key(name) {
            return Err(BrokerError::Invalid(
                "body contains reserved session field".into(),
            ));
        }
        body.insert(
            name.clone(),
            serde_json::Value::String(session_field(&session, field)?.to_owned()),
        );
    }
    Ok(Some(serde_json::Value::Object(body)))
}

pub(super) fn apply_secret(
    request: reqwest::RequestBuilder,
    secret: &SecretRequest<'_>,
) -> Result<reqwest::RequestBuilder, BrokerError> {
    match secret.injection {
        SecretInjection::Header { name, prefix, .. } => Ok(request.header(
            reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| BrokerError::Invalid("invalid secret header".into()))?,
            reqwest::header::HeaderValue::from_str(&format!("{prefix}{}", secret.secret))
                .map_err(|_| BrokerError::Invalid("invalid secret header".into()))?,
        )),
        SecretInjection::Basic { password, .. } => {
            Ok(request.basic_auth(secret.secret, Some(password)))
        }
        SecretInjection::Query { .. } => Ok(request),
        SecretInjection::Json {
            header_fields,
            headers,
            ..
        } => {
            let session: serde_json::Value = serde_json::from_str(secret.secret)
                .map_err(|_| BrokerError::Invalid("invalid JSON session".into()))?;
            let mut request = request;
            for (name, value) in headers {
                request = request.header(name, value);
            }
            for (name, field) in header_fields {
                let value = reqwest::header::HeaderValue::from_str(session_field(&session, field)?)
                    .map_err(|_| BrokerError::Invalid("invalid session header".into()))?;
                request = request.header(name, value);
            }
            Ok(request)
        }
        SecretInjection::Cookies { .. } => {
            let values: HashMap<String, String> = serde_json::from_str(secret.secret)
                .map_err(|_| BrokerError::Invalid("invalid secret cookies".into()))?;
            if values.is_empty()
                || secret
                    .allowed_cookie_names
                    .iter()
                    .any(|name| !values.contains_key(name))
                || values.keys().any(|name| {
                    !secret
                        .allowed_cookie_names
                        .iter()
                        .any(|allowed| allowed == name)
                })
                || values.values().any(|value| {
                    value.is_empty()
                        || value.len() > 4096
                        || value
                            .chars()
                            .any(|character| matches!(character, ';' | '\r' | '\n'))
                })
            {
                return Err(BrokerError::Invalid("invalid secret cookies".into()));
            }
            let mut request = request;
            if let Some(headers) = secret.injection.fixed_headers() {
                for (name, value) in headers {
                    request = request.header(name, value);
                }
            }
            if let Some(mirror) = secret.injection.cookie_header() {
                request = request.header(
                    &mirror.header,
                    values
                        .get(&mirror.cookie)
                        .ok_or_else(|| BrokerError::Invalid("missing mirrored cookie".into()))?,
                );
            }
            let cookies = values
                .into_iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            Ok(request.header(reqwest::header::COOKIE, cookies))
        }
    }
}
