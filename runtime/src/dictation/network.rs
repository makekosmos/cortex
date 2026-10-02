// Network factory для AI HTTP клиентов. Поддерживает выбор DNS resolver'а
// (системный / Cloudflare DoH / Google DoH / custom DoH URL).
//
// Scope: ТОЛЬКО dictation HTTP (Phase 1, см. forbidden.md → Dictation).
// Generic API — `build_client(profile)` спроектирована переиспользуемой
// для будущих AI-провайдеров.

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use thiserror::Error;
use tracing::{debug, warn};

use super::config::NetworkProfile;

/// Ошибки сборки HTTP клиента. Отделены от reqwest::Error чтобы можно было
/// выдавать понятные сообщения о невалидной конфигурации (custom DoH URL,
/// proxy URL) ДО реального сетевого запроса.
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("custom DoH URL невалидный: {0}")]
    CustomDohInvalid(String),
    #[error("http proxy URL невалидный: {0}")]
    ProxyInvalid(String),
}

/// Распарсенный custom DoH URL. Чистая структура без I/O — реальный DNS
/// lookup для не-литеральных хостов делается в `build_resolver`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCustomDoh {
    pub host: String,
    pub port: u16,
    pub path: String,
    /// `Some(ip)` если host в URL — IP-литерал (`https://1.1.1.1/dns-query`).
    /// Тогда DNS-резолва хоста не нужно.
    pub literal_ip: Option<IpAddr>,
}

/// Валидация и парсинг custom DoH URL. Чистая функция — не делает I/O.
///
/// Правила:
///   - схема ОБЯЗАНА быть `https://` (DoH по определению HTTPS);
///   - userinfo (`user:pass@host`) запрещён — DoH не использует;
///   - host обязателен;
///   - путь без `/dns-query` → автоматически добавляем (UX: юзер часто
///     даёт только хост);
///   - IP-литералы (v4 и v6 в `[..]`) парсятся в `literal_ip`.
pub fn validate_custom_doh_url(url: &str) -> Result<ParsedCustomDoh, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL пустой".into());
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "URL должен начинаться с https://".to_string())?;
    if rest.is_empty() {
        return Err("хост не задан".into());
    }
    if rest.contains('@') {
        return Err("userinfo (user:pass@host) в DoH URL не поддерживается".into());
    }
    // Разделяем на authority и path по первому `/`.
    let (authority, path_raw) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err("хост не задан".into());
    }
    // IPv6 в URL — в квадратных скобках: `[::1]:8443`.
    let (host_str, port_str) = if let Some(close) = authority.strip_prefix('[') {
        let end = close
            .find(']')
            .ok_or_else(|| "IPv6 host не закрыт `]`".to_string())?;
        let host = &close[..end];
        let after = &close[end + 1..];
        let port = if let Some(p) = after.strip_prefix(':') {
            Some(p)
        } else if after.is_empty() {
            None
        } else {
            return Err(format!("после IPv6 host неожидаемые символы: '{after}'"));
        };
        (host.to_string(), port)
    } else {
        // IPv4 / DNS hostname — `host` или `host:port`.
        match authority.rsplit_once(':') {
            Some((h, p)) if h.parse::<IpAddr>().is_err() && !h.is_empty() => {
                (h.to_string(), Some(p))
            }
            Some((h, p)) if h.parse::<IpAddr>().is_ok() => (h.to_string(), Some(p)),
            _ => (authority.to_string(), None),
        }
    };
    let port: u16 = match port_str {
        Some(p) => p.parse().map_err(|_| format!("неверный port: '{p}'"))?,
        None => 443,
    };
    let literal_ip = IpAddr::from_str(&host_str).ok();
    let path = if path_raw.is_empty() || path_raw == "/" {
        "/dns-query".to_string()
    } else {
        path_raw.to_string()
    };
    Ok(ParsedCustomDoh {
        host: host_str,
        port,
        path,
        literal_ip,
    })
}

/// Билдит reqwest клиент с timeout 60s + опциональный DoH resolver
/// + опциональный HTTP/SOCKS proxy. Proxy применяется ПОВЕРХ DNS-настройки:
///   сначала резолвится host (через выбранный resolver), потом подключение
///   идёт через proxy (если задан). Это покрывает три сценария РФ-обхода:
///   1. DoH only — DNS poisoning (Cloudflare 1.1.1.1).
///   2. Proxy only — IP/SNI block без DNS issue.
///   3. DoH + Proxy — двойная защита, если оба слоя проблемные.
pub fn build_client(
    profile: &NetworkProfile,
    http_proxy: Option<&str>,
) -> Result<reqwest::Client, NetworkError> {
    // Таймауты подобраны под Phase 2 auto-retry flow:
    //   - connect_timeout 6s — на оффлайне reqwest по умолчанию ждёт ВСЁ
    //     `.timeout(...)` (60s было) даже если соединение явно мертво. С
    //     явным connect_timeout fail происходит за 6с → attempts в очереди
    //     быстро инкрементятся, пользователь видит прогресс.
    //   - timeout 30s — реальная Groq транскрипция ~1-3с, 30s даёт запас на
    //     long upload (>30s аудио) без перехода в зависание.
    let builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .user_agent("Mundus/Mundus dictation");

    finish_client_builder(builder, profile, http_proxy)
}

/// Builds a client for large local model downloads. Unlike `build_client`, this
/// intentionally does not set a whole-request timeout: model bodies can be 1GB+
/// and must be allowed to stream for minutes while retaining connect timeout.
pub fn build_download_client(
    profile: &NetworkProfile,
    http_proxy: Option<&str>,
) -> Result<reqwest::Client, NetworkError> {
    let builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .user_agent("Mundus/Mundus dictation download");

    finish_client_builder(builder, profile, http_proxy)
}

fn finish_client_builder(
    mut builder: reqwest::ClientBuilder,
    profile: &NetworkProfile,
    http_proxy: Option<&str>,
) -> Result<reqwest::Client, NetworkError> {
    if let Some(resolver) = build_resolver(profile)? {
        debug!(?profile, "dictation: using custom DNS resolver");
        builder = builder.dns_resolver(resolver);
    }

    if let Some(url) = http_proxy {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            let proxy = reqwest::Proxy::all(trimmed)
                .map_err(|e| NetworkError::ProxyInvalid(e.to_string()))?;
            debug!(proxy = %trimmed, "dictation: applying HTTP proxy");
            builder = builder.proxy(proxy);
        }
    }

    Ok(builder.build()?)
}

fn build_resolver(
    profile: &NetworkProfile,
) -> Result<Option<Arc<HickoryDnsResolver>>, NetworkError> {
    use hickory_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
    use hickory_resolver::TokioAsyncResolver;

    let config = match profile {
        NetworkProfile::System => return Ok(None),
        NetworkProfile::CloudflareDoh => ResolverConfig::cloudflare_https(),
        NetworkProfile::GoogleDoh => ResolverConfig::google_https(),
        NetworkProfile::CustomDoh { url } => {
            let parsed = validate_custom_doh_url(url).map_err(NetworkError::CustomDohInvalid)?;
            // Для не-IP host'ов нам нужен IP чтобы построить NameServerConfig.
            // Резолвим через системный DNS однократно при build (bootstrap).
            // Если системный DNS тоже отказывает — фатальная ошибка
            // (custom DoH не может работать без знания IP резолвера).
            let ip = match parsed.literal_ip {
                Some(ip) => ip,
                None => {
                    use std::net::ToSocketAddrs;
                    let addrs = (parsed.host.as_str(), parsed.port)
                        .to_socket_addrs()
                        .map_err(|e| {
                            NetworkError::CustomDohInvalid(format!(
                                "не удалось резолвить host '{}' через системный DNS: {}",
                                parsed.host, e
                            ))
                        })?;
                    addrs.map(|s| s.ip()).next().ok_or_else(|| {
                        NetworkError::CustomDohInvalid(format!(
                            "системный DNS не вернул адресов для '{}'",
                            parsed.host
                        ))
                    })?
                }
            };
            let mut cfg = ResolverConfig::new();
            cfg.add_name_server(NameServerConfig {
                socket_addr: SocketAddr::new(ip, parsed.port),
                protocol: Protocol::Https,
                tls_dns_name: Some(parsed.host.clone()),
                trust_negative_responses: false,
                bind_addr: None,
                tls_config: None,
            });
            debug!(host = %parsed.host, ip = %ip, port = parsed.port, path = %parsed.path, "dictation: custom DoH ready");
            cfg
        }
    };

    // Короткие ResolverOpts чтобы DoH не висел минутами на оффлайне.
    let mut opts = ResolverOpts::default();
    opts.timeout = Duration::from_secs(3);
    opts.attempts = 2;
    let resolver = TokioAsyncResolver::tokio(config, opts);
    Ok(Some(Arc::new(HickoryDnsResolver(Arc::new(resolver)))))
}

/// Ошибки DNS lookup'а — выделены отдельно от NetworkError для probe stage
/// (system / DoH могут падать по разным причинам).
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("system DNS lookup для '{host}': {source}")]
    System {
        host: String,
        source: std::io::Error,
    },
    #[error("DoH resolver fail для '{host}': {msg}")]
    Doh { host: String, msg: String },
    #[error("неверный hostname '{0}'")]
    InvalidHost(String),
    #[error("конфиг DoH: {0}")]
    Config(#[from] NetworkError),
}

/// Резолв хоста через выбранный profile. Для System — `tokio::net::lookup_host`
/// (фактически getaddrinfo). Для DoH — прямой вызов hickory resolver. Используется
/// в `op_test_connectivity` для отдельной DNS-стадии диагностики.
pub async fn resolve_host(
    profile: &NetworkProfile,
    host: &str,
) -> Result<Vec<IpAddr>, ResolveError> {
    if matches!(profile, NetworkProfile::System) {
        let addrs =
            tokio::net::lookup_host((host, 443))
                .await
                .map_err(|e| ResolveError::System {
                    host: host.into(),
                    source: e,
                })?;
        return Ok(addrs.map(|s| s.ip()).collect());
    }
    let resolver = build_resolver(profile)?.expect("non-System profile must produce resolver");
    let name = Name::from_str(host).map_err(|_| ResolveError::InvalidHost(host.into()))?;
    let resolving = resolver.resolve(name);
    let addrs = resolving.await.map_err(|e| ResolveError::Doh {
        host: host.into(),
        msg: e.to_string(),
    })?;
    Ok(addrs.map(|s| s.ip()).collect())
}

pub struct HickoryDnsResolver(Arc<hickory_resolver::TokioAsyncResolver>);

impl Resolve for HickoryDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = self.0.clone();
        Box::pin(async move {
            let host = name.as_str().trim_end_matches('.').to_owned();
            let lookup = resolver.lookup_ip(host.clone()).await.map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    warn!(host = %host, error = %e, "dictation: DoH lookup failed");
                    Box::new(e)
                },
            )?;
            let addrs: Vec<SocketAddr> = lookup.iter().map(|ip| SocketAddr::new(ip, 0)).collect();
            let iter: Addrs = Box::new(addrs.into_iter());
            Ok(iter)
        })
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // validate_custom_doh_url
    // ------------------------------------------------------------------

    #[test]
    fn validate_empty_url_errors() {
        let r = validate_custom_doh_url("");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("пустой"));
    }

    #[test]
    fn validate_whitespace_url_errors() {
        assert!(validate_custom_doh_url("   ").is_err());
    }

    #[test]
    fn validate_no_scheme_errors() {
        let r = validate_custom_doh_url("comss.dns.controld.com/dns-query");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("https://"));
    }

    #[test]
    fn validate_http_scheme_errors() {
        let r = validate_custom_doh_url("http://example.com/dns-query");
        assert!(r.is_err(), "http (без s) запрещён для DoH");
    }

    #[test]
    fn validate_userinfo_errors() {
        let r = validate_custom_doh_url("https://user:pass@example.com/dns-query");
        assert!(r.is_err());
        assert!(r.unwrap_err().to_lowercase().contains("userinfo"));
    }

    #[test]
    fn validate_no_host_errors() {
        let r = validate_custom_doh_url("https://");
        assert!(r.is_err());
    }

    #[test]
    fn validate_valid_hostname() {
        let p = validate_custom_doh_url("https://comss.dns.controld.com/dns-query").unwrap();
        assert_eq!(p.host, "comss.dns.controld.com");
        assert_eq!(p.port, 443);
        assert_eq!(p.path, "/dns-query");
        assert!(p.literal_ip.is_none());
    }

    #[test]
    fn validate_auto_adds_dns_query_path() {
        let p = validate_custom_doh_url("https://comss.dns.controld.com").unwrap();
        assert_eq!(p.path, "/dns-query");
    }

    #[test]
    fn validate_auto_adds_dns_query_for_root_path() {
        let p = validate_custom_doh_url("https://comss.dns.controld.com/").unwrap();
        assert_eq!(p.path, "/dns-query");
    }

    #[test]
    fn validate_preserves_custom_path() {
        let p = validate_custom_doh_url("https://example.com/custom/dns").unwrap();
        assert_eq!(p.path, "/custom/dns");
    }

    #[test]
    fn validate_custom_port() {
        let p = validate_custom_doh_url("https://example.com:8443/dns-query").unwrap();
        assert_eq!(p.port, 8443);
        assert_eq!(p.host, "example.com");
    }

    #[test]
    fn validate_invalid_port_errors() {
        let r = validate_custom_doh_url("https://example.com:notanumber/dns-query");
        assert!(r.is_err());
    }

    #[test]
    fn validate_ipv4_literal() {
        let p = validate_custom_doh_url("https://1.1.1.1/dns-query").unwrap();
        assert_eq!(p.host, "1.1.1.1");
        assert_eq!(p.literal_ip, Some("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn validate_ipv4_literal_with_port() {
        let p = validate_custom_doh_url("https://8.8.8.8:443/dns-query").unwrap();
        assert_eq!(p.host, "8.8.8.8");
        assert_eq!(p.port, 443);
        assert_eq!(p.literal_ip, Some("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn validate_ipv6_literal_bracketed() {
        let p = validate_custom_doh_url("https://[2606:4700:4700::1111]/dns-query").unwrap();
        assert_eq!(p.host, "2606:4700:4700::1111");
        assert_eq!(p.port, 443);
        assert!(p.literal_ip.is_some());
    }

    #[test]
    fn validate_ipv6_literal_with_port() {
        let p = validate_custom_doh_url("https://[::1]:8443/dns-query").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 8443);
    }

    #[test]
    fn validate_ipv6_unclosed_bracket_errors() {
        let r = validate_custom_doh_url("https://[::1/dns-query");
        assert!(r.is_err());
    }

    #[test]
    fn validate_trims_whitespace() {
        let p = validate_custom_doh_url("  https://example.com/dns-query  ").unwrap();
        assert_eq!(p.host, "example.com");
    }

    // ------------------------------------------------------------------
    // build_client — existing smoke tests (после смены типа ошибки)
    // ------------------------------------------------------------------

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
    fn build_client_custom_doh_with_literal_ip() {
        // Литеральный IP — НЕ требует системного DNS, безопасно для unit-теста.
        let client = build_client(
            &NetworkProfile::CustomDoh {
                url: "https://1.1.1.1/dns-query".into(),
            },
            None,
        );
        assert!(client.is_ok(), "literal IP custom DoH должен build'иться");
    }

    #[test]
    fn build_client_custom_doh_invalid_url_errors() {
        let r = build_client(
            &NetworkProfile::CustomDoh {
                url: "not a url".into(),
            },
            None,
        );
        match r {
            Err(NetworkError::CustomDohInvalid(_)) => {}
            other => panic!("expected CustomDohInvalid, got {other:?}"),
        }
    }

    #[test]
    fn build_client_custom_doh_http_scheme_errors() {
        let r = build_client(
            &NetworkProfile::CustomDoh {
                url: "http://1.1.1.1/dns-query".into(),
            },
            None,
        );
        assert!(matches!(r, Err(NetworkError::CustomDohInvalid(_))));
    }

    #[test]
    fn build_client_with_http_proxy() {
        let client = build_client(&NetworkProfile::System, Some("http://127.0.0.1:8888"));
        assert!(client.is_ok());
    }

    #[test]
    fn build_client_with_socks5_proxy() {
        let client = build_client(
            &NetworkProfile::CloudflareDoh,
            Some("socks5://127.0.0.1:1080"),
        );
        assert!(client.is_ok());
    }

    #[test]
    fn build_client_empty_proxy_string_ignored() {
        let client = build_client(&NetworkProfile::System, Some("   "));
        assert!(client.is_ok());
    }

    #[test]
    fn build_client_invalid_proxy_url_errors() {
        let r = build_client(&NetworkProfile::System, Some("not a url"));
        assert!(matches!(r, Err(NetworkError::ProxyInvalid(_))));
    }

    // ------------------------------------------------------------------
    // Integration tests с реальной сетью. Запускаются только вручную
    // через `cargo test integration_ -- --ignored --nocapture`.
    // Цель: проверить какие DoH провайдеры реально работают для Groq
    // из текущей сети (РФ или иная).
    // ------------------------------------------------------------------

    async fn try_doh(name: &str, url: &str) -> (bool, String) {
        let client = match build_client(&NetworkProfile::CustomDoh { url: url.into() }, None) {
            Ok(c) => c,
            Err(e) => return (false, format!("build_client failed: {e}")),
        };
        let start = std::time::Instant::now();
        match client
            .head("https://api.groq.com/openai/v1/models")
            .send()
            .await
        {
            Ok(r) => {
                let status = r.status().as_u16();
                let ms = start.elapsed().as_millis();
                let ok = status < 400;
                (ok, format!("{name}: HTTP {status} за {ms}ms"))
            }
            Err(e) => (
                false,
                format!("{name}: {e} за {}ms", start.elapsed().as_millis()),
            ),
        }
    }

    #[tokio::test]
    #[ignore = concat!("diagnostic probe: needs live internet access to public DoH providers and ","prints a report instead of asserting")]
    async fn integration_dns_providers_matrix() {
        // Матрица DoH провайдеров — посмотреть кто реально пускает к Groq
        // из текущей сети. NOT run in CI (требует internet + время).
        let cases: &[(&str, &str)] = &[
            ("System", "system://placeholder"), // обрабатывается отдельно ниже
            ("Cloudflare", "https://1.1.1.1/dns-query"),
            ("Google", "https://dns.google/dns-query"),
            ("Quad9", "https://dns.quad9.net/dns-query"),
            ("AdGuard", "https://dns.adguard-dns.com/dns-query"),
            ("ControlD-free", "https://freedns.controld.com/p2"),
            ("Mullvad", "https://dns.mullvad.net/dns-query"),
            ("malw.link", "https://dns.malw.link/dns-query"),
            ("NextDNS-anon", "https://anycast.dns.nextdns.io/"),
        ];
        let mut report: Vec<String> = Vec::new();

        // System DNS sanity check
        let sys_client = build_client(&NetworkProfile::System, None).unwrap();
        let start = std::time::Instant::now();
        match sys_client
            .head("https://api.groq.com/openai/v1/models")
            .send()
            .await
        {
            Ok(r) => report.push(format!(
                "System: HTTP {} за {}ms",
                r.status().as_u16(),
                start.elapsed().as_millis()
            )),
            Err(e) => report.push(format!("System: {e}")),
        }

        for (name, url) in cases.iter().skip(1) {
            let (_ok, msg) = try_doh(name, url).await;
            report.push(msg);
        }

        println!("\n\n=== Groq DoH matrix ===");
        for line in &report {
            println!("  {line}");
        }
        println!("=======================\n");
    }

    #[tokio::test]
    #[ignore = concat!("diagnostic probe: needs live internet access to public DoH providers and ","prints a report instead of asserting")]
    async fn integration_dns_resolve_matrix() {
        // Изолируем DNS-резолв (без HTTPS к Groq). Покажет где валится:
        // на DoH connect или дальше.
        let cases: &[(&str, NetworkProfile)] = &[
            ("System", NetworkProfile::System),
            ("Cloudflare", NetworkProfile::CloudflareDoh),
            ("Google", NetworkProfile::GoogleDoh),
            (
                "Quad9",
                NetworkProfile::CustomDoh {
                    url: "https://dns.quad9.net/dns-query".into(),
                },
            ),
            (
                "AdGuard",
                NetworkProfile::CustomDoh {
                    url: "https://dns.adguard-dns.com/dns-query".into(),
                },
            ),
            (
                "Mullvad",
                NetworkProfile::CustomDoh {
                    url: "https://dns.mullvad.net/dns-query".into(),
                },
            ),
            (
                "ControlD-free-p2",
                NetworkProfile::CustomDoh {
                    url: "https://freedns.controld.com/p2".into(),
                },
            ),
            (
                "malw.link",
                NetworkProfile::CustomDoh {
                    url: "https://dns.malw.link/dns-query".into(),
                },
            ),
        ];

        println!("\n=== DNS resolve api.groq.com matrix ===");
        for (name, profile) in cases {
            let start = std::time::Instant::now();
            let result = resolve_host(profile, "api.groq.com").await;
            let ms = start.elapsed().as_millis();
            match result {
                Ok(ips) if !ips.is_empty() => {
                    let ip_str = ips
                        .iter()
                        .take(3)
                        .map(|ip| ip.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    println!("  {name:18} OK   {ms:>5}ms → {ip_str}");
                }
                Ok(_) => println!("  {name:18} EMPTY {ms:>5}ms"),
                Err(e) => println!("  {name:18} FAIL {ms:>5}ms → {e}"),
            }
        }
        println!("======================================\n");
    }

    #[tokio::test]
    #[ignore = "needs live internet access to dns.malw.link and api.groq.com"]
    async fn integration_user_provided_doh_malwlink() {
        // Конкретно проверяем https://dns.malw.link/dns-query — юзер просил.
        let (ok, msg) = try_doh("malw.link", "https://dns.malw.link/dns-query").await;
        println!("\n=== malw.link DoH ===\n  {msg}\n=====================\n");
        if !ok {
            panic!("malw.link DoH не пропустил к Groq: {msg}");
        }
    }
}
