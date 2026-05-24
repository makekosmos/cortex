// Network factory для AI HTTP клиентов. Поддерживает выбор DNS resolver'а
// (системный / Cloudflare DoH / Google DoH / custom DoH URL).
//
// Scope: ТОЛЬКО dictation HTTP (Phase 1, см. forbidden.md → Dictation).
// Generic API — `build_client(profile)` спроектирована переиспользуемой
// для будущих AI-провайдеров.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::config::NetworkProfile;

/// Билдит reqwest клиент с timeout 60s + опциональный DoH resolver
/// + опциональный HTTP/SOCKS proxy. Proxy применяется ПОВЕРХ DNS-настройки:
/// сначала резолвится host (через выбранный resolver), потом подключение
/// идёт через proxy (если задан). Это покрывает три сценария РФ-обхода:
///   1. DoH only — DNS poisoning (Cloudflare 1.1.1.1).
///   2. Proxy only — IP/SNI block без DNS issue.
///   3. DoH + Proxy — двойная защита, если оба слоя проблемные.
pub fn build_client(
    profile: &NetworkProfile,
    http_proxy: Option<&str>,
) -> reqwest::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("Kosmos/Kepler dictation");

    if let Some(resolver) = build_resolver(profile) {
        builder = builder.dns_resolver(resolver);
    }

    if let Some(url) = http_proxy {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            let proxy = reqwest::Proxy::all(trimmed)?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build()
}

fn build_resolver(profile: &NetworkProfile) -> Option<Arc<HickoryDnsResolver>> {
    use hickory_resolver::config::{ResolverConfig, ResolverOpts};
    use hickory_resolver::TokioAsyncResolver;

    let config = match profile {
        NetworkProfile::System => return None,
        NetworkProfile::CloudflareDoh => ResolverConfig::cloudflare_https(),
        NetworkProfile::GoogleDoh => ResolverConfig::google_https(),
        NetworkProfile::CustomDoh { url: _ } => {
            // TODO Phase 1.5: parse custom DoH URL → ServerName + IPs.
            // Сейчас fallback на Cloudflare (всё равно DoH), чтобы UI ручка не падала.
            // Логируем warning один раз через eprintln (нет state'а для once-флага).
            eprintln!(
                "[dictation::network] custom DoH URL пока не поддерживается, fallback на Cloudflare DoH"
            );
            ResolverConfig::cloudflare_https()
        }
    };

    let resolver = TokioAsyncResolver::tokio(config, ResolverOpts::default());
    Some(Arc::new(HickoryDnsResolver(Arc::new(resolver))))
}

pub struct HickoryDnsResolver(Arc<hickory_resolver::TokioAsyncResolver>);

impl Resolve for HickoryDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = self.0.clone();
        Box::pin(async move {
            let host = name.as_str().trim_end_matches('.').to_owned();
            let lookup = resolver
                .lookup_ip(host)
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
            let addrs: Vec<SocketAddr> = lookup.iter().map(|ip| SocketAddr::new(ip, 0)).collect();
            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_client_system_profile() {
        let client = build_client(&NetworkProfile::System, None);
        assert!(client.is_ok(), "system profile should build");
    }

    #[test]
    fn build_client_cloudflare_doh() {
        let client = build_client(&NetworkProfile::CloudflareDoh, None);
        assert!(client.is_ok(), "cloudflare DoH profile should build");
    }

    #[test]
    fn build_client_google_doh() {
        let client = build_client(&NetworkProfile::GoogleDoh, None);
        assert!(client.is_ok(), "google DoH profile should build");
    }

    #[test]
    fn build_client_custom_doh_fallback() {
        let client = build_client(
            &NetworkProfile::CustomDoh {
                url: "https://comss.dns.controld.com/dns-query".into(),
            },
            None,
        );
        assert!(client.is_ok(), "custom DoH profile should build (fallback)");
    }

    #[test]
    fn build_client_with_http_proxy() {
        let client = build_client(&NetworkProfile::System, Some("http://127.0.0.1:8888"));
        assert!(client.is_ok(), "client with http proxy should build");
    }

    #[test]
    fn build_client_with_socks5_proxy() {
        let client = build_client(&NetworkProfile::CloudflareDoh, Some("socks5://127.0.0.1:1080"));
        assert!(client.is_ok(), "client with socks5 proxy should build");
    }

    #[test]
    fn build_client_empty_proxy_string_ignored() {
        let client = build_client(&NetworkProfile::System, Some("   "));
        assert!(client.is_ok(), "empty/whitespace proxy should be ignored");
    }

    #[test]
    fn build_client_invalid_proxy_url_errors() {
        let client = build_client(&NetworkProfile::System, Some("not a url"));
        assert!(client.is_err(), "invalid proxy URL must error");
    }
}
