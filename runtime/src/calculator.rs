use std::{
    collections::HashMap,
    error::Error,
    path::Path,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use quick_xml::{events::Event, Reader};
use serde::{Deserialize, Serialize};

const MAX_QUERY_CHARS: usize = 256;
const EVALUATION_TIMEOUT: Duration = Duration::from_millis(50);
const RATE_CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const RATE_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const CBR_DAILY_RATES_URL: &str = "https://www.cbr.ru/scripts/XML_daily.asp";
const RATE_CACHE_FILE: &str = "calculator-exchange-rates.json";

static RATE_REFRESH_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NormalizedQuery {
    pub evaluation: String,
    pub display_expression: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ExchangeRates {
    fetched_at: u64,
    rates: HashMap<String, f64>,
}

#[derive(Clone)]
struct ExchangeRateHandler(ExchangeRates);

impl fend_core::ExchangeRateFnV2 for ExchangeRateHandler {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _options: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn Error + Send + Sync + 'static>> {
        self.0.rates.get(currency).copied().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("currency exchange rate for {currency} is unknown"),
            )
            .into()
        })
    }
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

/// Evaluates launcher-friendly expressions without mutating state or accessing the network.
/// Invalid input is normal search text, so it quietly returns `None`.
pub fn evaluate_preview(query: &str) -> Option<String> {
    evaluate_preview_with_rates(&normalize_query(query).evaluation, None)
}

pub(crate) fn normalize_query(query: &str) -> NormalizedQuery {
    let input = query.trim();
    if let Some((amount, source, target)) = normalize_russian_currency_query(input) {
        return NormalizedQuery {
            evaluation: format!("{amount} {source} to {target}"),
            display_expression: format!("{amount} {source}"),
        };
    }

    NormalizedQuery {
        evaluation: input.to_string(),
        display_expression: currency_source_expression(input)
            .unwrap_or(input)
            .to_string(),
    }
}

fn normalize_russian_currency_query(input: &str) -> Option<(String, &'static str, &'static str)> {
    let tokens = input.split_whitespace().collect::<Vec<_>>();
    if tokens.len() != 4 || tokens[2].to_lowercase() != "в" {
        return None;
    }
    let amount = tokens[0].replace(',', ".");
    amount.parse::<f64>().ok()?;
    Some((
        amount,
        russian_currency_code(tokens[1])?,
        russian_currency_code(tokens[3])?,
    ))
}

fn russian_currency_code(token: &str) -> Option<&'static str> {
    let token = token
        .trim_matches(|ch: char| !ch.is_alphabetic())
        .to_lowercase();
    match token.as_str() {
        "usd" | "доллар" | "доллара" | "долларов" | "доллары" | "долларах" => {
            Some("USD")
        }
        "rub" | "руб" | "рубль" | "рубля" | "рублей" | "рубли" | "рублях" => {
            Some("RUB")
        }
        "eur" | "евро" => Some("EUR"),
        "cny" | "юань" | "юаня" | "юаней" | "юани" | "юанях" => Some("CNY"),
        _ => None,
    }
}

fn currency_source_expression(input: &str) -> Option<&str> {
    if !looks_like_currency_conversion(input) {
        return None;
    }
    let lower = input.to_ascii_lowercase();
    [" to ", " in "]
        .into_iter()
        .find_map(|separator| lower.find(separator).map(|index| input[..index].trim()))
}

pub(crate) fn evaluate_preview_with_rates(
    query: &str,
    rates: Option<ExchangeRates>,
) -> Option<String> {
    let input = query.trim();
    if input.is_empty() || input.chars().count() > MAX_QUERY_CHARS || !looks_like_calculation(input)
    {
        return None;
    }

    let mut context = fend_core::Context::new();
    let evaluation_input = if rates.is_some() {
        format!("@noapprox ({input}) to 2 dp")
    } else {
        input.to_string()
    };
    if let Some(rates) = rates {
        context.set_exchange_rate_handler_v2(ExchangeRateHandler(rates));
    }
    let deadline = Deadline(Instant::now() + EVALUATION_TIMEOUT);
    let result = fend_core::evaluate_preview_with_interrupt(&evaluation_input, &context, &deadline);
    let output = result.get_main_result().trim();

    (!output.is_empty() && output != input).then(|| output.to_string())
}

pub(crate) async fn exchange_rates_for_query(
    query: &str,
    data_dir: &Path,
) -> Option<ExchangeRates> {
    exchange_rates_for_query_from(query, data_dir, CBR_DAILY_RATES_URL).await
}

async fn exchange_rates_for_query_from(
    query: &str,
    data_dir: &Path,
    endpoint: &str,
) -> Option<ExchangeRates> {
    if !looks_like_currency_conversion(query) {
        return None;
    }

    let cache_path = data_dir.join(RATE_CACHE_FILE);
    let lock = RATE_REFRESH_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = lock.lock().await;
    let cached = load_rate_cache(&cache_path);
    if cached.as_ref().is_some_and(is_fresh) {
        return cached;
    }

    match fetch_rates(endpoint).await {
        Ok(rates) => {
            if let Err(error) = store_rate_cache(&cache_path, &rates) {
                eprintln!("[calculator] failed to persist exchange-rate cache: {error}");
            }
            Some(rates)
        }
        Err(error) => {
            eprintln!("[calculator] exchange-rate refresh failed, using stale cache: {error}");
            cached
        }
    }
}

fn looks_like_currency_conversion(input: &str) -> bool {
    input
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|token| token.len() == 3)
        .take(2)
        .count()
        >= 2
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_fresh(rates: &ExchangeRates) -> bool {
    now_timestamp().saturating_sub(rates.fetched_at) <= RATE_CACHE_MAX_AGE.as_secs()
}

fn load_rate_cache(path: &Path) -> Option<ExchangeRates> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn store_rate_cache(path: &Path, rates: &ExchangeRates) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(rates)?)?;
    Ok(())
}

async fn fetch_rates(endpoint: &str) -> Result<ExchangeRates, Box<dyn Error + Send + Sync>> {
    let bytes = reqwest::Client::builder()
        .connect_timeout(RATE_REQUEST_TIMEOUT)
        .timeout(RATE_REQUEST_TIMEOUT)
        .build()?
        .get(endpoint)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    parse_cbr_rates(&bytes)
}

fn parse_cbr_rates(xml: &[u8]) -> Result<ExchangeRates, Box<dyn Error + Send + Sync>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut field = None;
    let mut code = None;
    let mut nominal = None;
    let mut value = None;
    let mut rates = HashMap::from([("RUB".to_string(), 1.0)]);

    loop {
        match reader.read_event()? {
            Event::Start(tag) => {
                field = match tag.name().as_ref() {
                    b"CharCode" => Some("code"),
                    b"Nominal" => Some("nominal"),
                    b"Value" => Some("value"),
                    _ => None,
                };
            }
            Event::Text(text) => {
                let Some(field) = field else { continue };
                let bytes: &[u8] = text.as_ref();
                let text = std::str::from_utf8(bytes).map_err(|error| {
                    format!(
                        "invalid text for {field}: {error}; bytes={:?}",
                        &bytes[..bytes.len().min(8)]
                    )
                })?;
                let text = text.trim();
                match field {
                    "code" => code = Some(text.to_string()),
                    "nominal" => nominal = Some(text.parse::<f64>()?),
                    "value" => value = Some(text.replace(',', ".").parse::<f64>()?),
                    _ => {}
                }
            }
            Event::End(tag) => {
                if tag.name().as_ref() == b"Valute" {
                    if let (Some(code), Some(nominal), Some(value)) =
                        (code.take(), nominal.take(), value.take())
                    {
                        if nominal > 0.0 && value > 0.0 {
                            rates.insert(code, nominal / value);
                        }
                    }
                }
                field = None;
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if rates.len() < 2 {
        return Err("Bank of Russia response contained no exchange rates".into());
    }
    Ok(ExchangeRates {
        fetched_at: now_timestamp(),
        rates,
    })
}

fn looks_like_calculation(input: &str) -> bool {
    let has_digit = input.chars().any(|c| c.is_ascii_digit());
    let has_syntax = input.chars().any(|c| {
        c.is_ascii_alphabetic()
            || matches!(
                c,
                '+' | '-' | '*' | '/' | '^' | '%' | '(' | ')' | '@' | '×' | '÷'
            )
    });
    has_digit && has_syntax
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::GET, MockServer};

    const CBR_FIXTURE: &str = r#"<?xml version="1.0" encoding="windows-1251"?>
<ValCurs Date="15.07.2026" name="Foreign Currency Market">
  <Valute ID="R01235"><Nominal>1</Nominal><CharCode>USD</CharCode><Value>90,0000</Value></Valute>
  <Valute ID="R01239"><Nominal>1</Nominal><CharCode>EUR</CharCode><Value>100,0000</Value></Valute>
  <Valute ID="R01060"><Nominal>10</Nominal><CharCode>AMD</CharCode><Value>2,2500</Value></Valute>
</ValCurs>"#;

    #[test]
    fn evaluates_launcher_calculator_scenarios() {
        assert_eq!(evaluate_preview("1200 * 1.2").as_deref(), Some("1440"));
        assert_eq!(evaluate_preview("15% of 4500").as_deref(), Some("675"));
        assert!(evaluate_preview("10 km to miles")
            .is_some_and(|result| result.contains("6.2137") && result.contains("mile")));
        assert!(evaluate_preview("@2026-07-15 + 1 day")
            .is_some_and(|result| result.contains("16 July 2026")));
    }

    #[test]
    fn ignores_search_text_invalid_and_oversized_input() {
        assert_eq!(evaluate_preview("settings"), None);
        assert_eq!(evaluate_preview("2 +"), None);
        assert_eq!(evaluate_preview("100 USD to RUB"), None);
        assert_eq!(
            evaluate_preview(&format!("1+{}", "1".repeat(MAX_QUERY_CHARS))),
            None
        );
    }

    #[test]
    fn parses_nominals_and_evaluates_currency_conversion() {
        let rates = parse_cbr_rates(CBR_FIXTURE.as_bytes()).unwrap();
        assert_eq!(rates.rates["RUB"], 1.0);
        assert_eq!(rates.rates["USD"], 1.0 / 90.0);
        assert_eq!(rates.rates["AMD"], 10.0 / 2.25);
        let result = evaluate_preview_with_rates("100 USD to RUB", Some(rates.clone()));
        assert!(
            result
                .as_deref()
                .is_some_and(|result| result == "9000.00 RUB"),
            "unexpected currency result: {result:?}"
        );

        let normalized = normalize_query("900 долларов в рублях");
        assert_eq!(normalized.evaluation, "900 USD to RUB");
        assert_eq!(normalized.display_expression, "900 USD");
        assert_eq!(
            evaluate_preview_with_rates(&normalized.evaluation, Some(rates)).as_deref(),
            Some("81000.00 RUB")
        );
        assert_eq!(
            normalize_query("100 USD to RUB").display_expression,
            "100 USD"
        );
        assert_eq!(
            normalize_query("1200 * 1.2").display_expression,
            "1200 * 1.2"
        );
    }

    #[tokio::test]
    async fn caches_daily_rates_and_skips_network_for_ordinary_calculations() {
        let server = MockServer::start_async().await;
        let endpoint = server.url("/rates");
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/rates");
                then.status(200).body(CBR_FIXTURE);
            })
            .await;
        let data_dir = tempfile::tempdir().unwrap();

        assert!(
            exchange_rates_for_query_from("100 USD to RUB", data_dir.path(), &endpoint)
                .await
                .is_some()
        );
        assert!(
            exchange_rates_for_query_from("50 EUR to RUB", data_dir.path(), &endpoint)
                .await
                .is_some()
        );
        assert!(
            exchange_rates_for_query_from("10 km to miles", data_dir.path(), &endpoint)
                .await
                .is_none()
        );
        assert_eq!(mock.hits_async().await, 1);
    }

    #[tokio::test]
    async fn falls_back_to_stale_cache_when_refresh_fails() {
        let data_dir = tempfile::tempdir().unwrap();
        let cache_path = data_dir.path().join(RATE_CACHE_FILE);
        let mut stale = parse_cbr_rates(CBR_FIXTURE.as_bytes()).unwrap();
        stale.fetched_at = 0;
        store_rate_cache(&cache_path, &stale).unwrap();

        let loaded = exchange_rates_for_query_from(
            "100 USD to RUB",
            data_dir.path(),
            "http://127.0.0.1:1/rates",
        )
        .await
        .unwrap();
        assert_eq!(loaded.rates["USD"], 1.0 / 90.0);
    }

    #[tokio::test]
    #[ignore = "live Bank of Russia endpoint"]
    async fn live_cbr_rates_smoke() {
        let data_dir = tempfile::tempdir().unwrap();
        let rates = exchange_rates_for_query("100 USD to RUB", data_dir.path())
            .await
            .expect("Bank of Russia rates");
        assert!(rates.rates.contains_key("USD"));
        assert!(evaluate_preview_with_rates("100 USD to RUB", Some(rates)).is_some());
    }
}
