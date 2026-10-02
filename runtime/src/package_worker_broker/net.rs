//! Broker HTTPS fetch with origin validation and response bounds.

use super::secrets::{apply_secret, inject_json_body, request_body};
use super::*;
use crate::package_manifest::IntegrationRequestMethod;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub async fn fetch(config: &BrokerConfig, raw_url: &str) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret(config, raw_url, None).await
}

pub async fn fetch_with_secret(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret_json(config, raw_url, secret, None).await
}

pub async fn fetch_with_secret_json(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
    body: Option<&serde_json::Value>,
) -> Result<Vec<u8>, BrokerError> {
    fetch_with_secret_json_limit(config, raw_url, secret, body, MAX_BYTES).await
}

pub(crate) async fn fetch_with_secret_json_limit(
    config: &BrokerConfig,
    raw_url: &str,
    secret: Option<SecretRequest<'_>>,
    body: Option<&serde_json::Value>,
    limit: usize,
) -> Result<Vec<u8>, BrokerError> {
    if limit == 0 || limit > MAX_NETWORK_RESPONSE {
        return Err(BrokerError::Invalid("invalid response limit".into()));
    }
    let method = secret
        .as_ref()
        .map(|request| request.injection.request_method())
        .unwrap_or_default();
    let injected_body = inject_json_body(body, secret.as_ref())?;
    let body = request_body(method, injected_body.as_ref().or(body))?;
    let mut url = validate_url(config, raw_url)?;
    let mut redirect_count = 0;
    loop {
        let host = url
            .host_str()
            .ok_or_else(|| BrokerError::Invalid("missing host".into()))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| BrokerError::Invalid("missing port".into()))?;
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await?.collect();
        let addr = addrs
            .into_iter()
            .find(|a| {
                #[cfg(feature = "package-worker-fixture")]
                if config.allow_local_test_origin && a.ip().is_loopback() {
                    return true;
                }
                !is_blocked_ip(a.ip())
            })
            .ok_or_else(|| BrokerError::Invalid("host resolves to blocked address".into()))?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .resolve(host, addr)
            .build()?;
        let mut request_url = url.clone();
        if let Some(SecretRequest {
            injection: SecretInjection::Query { parameter, .. },
            secret,
            ..
        }) = secret.as_ref()
        {
            request_url.query_pairs_mut().append_pair(parameter, secret);
        }
        let mut request = match method {
            IntegrationRequestMethod::Get => client.get(request_url.clone()),
            IntegrationRequestMethod::PostJson => client
                .post(request_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone().unwrap_or_default()),
        };
        if let Some(secret) = secret.as_ref() {
            if !secret.injection.origins().iter().any(|origin| {
                reqwest::Url::parse(origin).is_ok_and(|allowed| allowed.origin() == url.origin())
            }) {
                return Err(BrokerError::Invalid("secret origin denied".into()));
            }
            request = apply_secret(request, secret)?;
        }
        let response = request.send().await?;
        if response.status().is_redirection() {
            if redirect_count == 3 {
                return Err(BrokerError::Invalid("too many redirects".into()));
            }
            redirect_count += 1;
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| BrokerError::Invalid("redirect missing location".into()))?;
            url = validate_url(
                config,
                url.join(location)
                    .map_err(|e| BrokerError::Invalid(e.to_string()))?
                    .as_str(),
            )?;
            continue;
        }
        if !response.status().is_success() {
            return Err(BrokerError::Invalid(format!(
                "HTTP status {}",
                response.status()
            )));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(BrokerError::Invalid("response exceeds limit".into()));
        }
        let mut out = Vec::new();
        let mut stream = response;
        while let Some(chunk) = stream.chunk().await? {
            if out.len() + chunk.len() > limit {
                return Err(BrokerError::Invalid("response exceeds limit".into()));
            }
            out.extend_from_slice(&chunk);
        }
        return Ok(out);
    }
}

pub(super) fn validate_url(config: &BrokerConfig, raw: &str) -> Result<reqwest::Url, BrokerError> {
    let url = reqwest::Url::parse(raw).map_err(|e| BrokerError::Invalid(e.to_string()))?;
    if (url.scheme() != "https"
        && !(cfg!(feature = "package-worker-fixture") && {
            #[cfg(feature = "package-worker-fixture")]
            {
                config.allow_local_test_origin
                    && url.scheme() == "http"
                    && url.host_str().is_some_and(is_loopback_host)
            }
            #[cfg(not(feature = "package-worker-fixture"))]
            {
                false
            }
        }))
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(BrokerError::Invalid(
            "only credential-free HTTPS URLs are allowed".into(),
        ));
    }
    if url
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case("localhost"))
        || !config
            .allowed_origins
            .contains(&url.origin().ascii_serialization())
    {
        return Err(BrokerError::Invalid("origin is not granted".into()));
    }
    Ok(url)
}

pub(super) fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_multicast()
                || v.is_unspecified()
                || v.octets()[0] == 0
        }
        IpAddr::V6(v) => {
            v.to_ipv4_mapped()
                .is_some_and(|v4| is_blocked_ip(v4.into()))
                || v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || ((v.segments()[0] & 0xfe00) == 0xfc00)
                || ((v.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

pub(super) fn is_loopback_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}
