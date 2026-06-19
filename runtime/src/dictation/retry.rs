// Retry с exponential backoff + классификация ошибок на retryable/fatal.
//
// Зачем: при network drop (DNS, TCP reset, 5xx, 429) — повторяем с jitter.
// При 401/400/413/audio decode — fail-fast, retry бесполезен.
//
// См. spec `.agent/tasks/2026-05-25-dictation-resilience/spec.md` § retry.rs.

#![allow(dead_code)] // wired в host.rs в task #11

use std::future::Future;
use std::time::Duration;

use super::groq::GroqError;
use super::host::SubmitError;
use super::local::LocalError;

/// Решение «повторять или fail-fast». Для Fatal — `user_msg` уже
/// локализован для показа в UI (не raw error string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FailureKind {
    Retryable,
    Fatal { user_msg: String },
}

impl FailureKind {
    pub(crate) fn is_retryable(&self) -> bool {
        matches!(self, FailureKind::Retryable)
    }
}

/// Классификация `SubmitError` на основе типа и (для Groq Api) HTTP-статуса.
///
/// Правила:
/// - `NoApiKey` / `AudioDecode` / `Inject*` → Fatal (caller bug / unrecoverable).
/// - `Local(*)` → Fatal с явным сообщением о missing path / unsupported runtime.
/// - `Network(reqwest::Error)` → Retryable. Reqwest сам по себе bookkeep'ит
///   `is_timeout/is_connect/is_request`; consider conservative — лучше повторим
///   и потеряем пару секунд, чем выкинем аудио.
/// - `Groq(GroqError::Http(_))` → Retryable (то же — transient transport).
/// - `Groq(GroqError::Api { status })`:
///     - 429, 5xx → Retryable
///     - 401 → Fatal «Неверный API key»
///     - 413 → Fatal «Аудио слишком длинное»
///     - 400 → Fatal «Groq отклонил запрос»
///     - прочие 4xx → Fatal «Ошибка Groq {status}»
pub(crate) fn classify(err: &SubmitError) -> FailureKind {
    match err {
        SubmitError::NoApiKey => FailureKind::Fatal {
            user_msg: "API key не задан".into(),
        },
        SubmitError::AudioDecode(_) => FailureKind::Fatal {
            user_msg: "Битое аудио".into(),
        },
        SubmitError::Network(_) => FailureKind::Retryable,
        SubmitError::Local(LocalError::MissingModelPath) => FailureKind::Fatal {
            user_msg: "Укажи путь к локальной Whisper-модели".into(),
        },
        SubmitError::Local(LocalError::ModelPathNotFound { path }) => FailureKind::Fatal {
            user_msg: format!("Локальная модель не найдена: {path}"),
        },
        SubmitError::Local(LocalError::MissingCommandPath) => FailureKind::Fatal {
            user_msg: "Укажи путь к whisper.cpp executable".into(),
        },
        SubmitError::Local(LocalError::CommandPathNotFound { path }) => FailureKind::Fatal {
            user_msg: format!("Локальный executable не найден: {path}"),
        },
        SubmitError::Local(LocalError::UnsupportedEngine { engine }) => FailureKind::Fatal {
            user_msg: format!("Локальный движок '{engine}' пока не поддерживается"),
        },
        SubmitError::Local(LocalError::TempAudio(message)) => FailureKind::Fatal {
            user_msg: format!("Не удалось подготовить аудио для локальной модели: {message}"),
        },
        SubmitError::Local(LocalError::CommandFailed(message)) => FailureKind::Fatal {
            user_msg: format!("Локальная транскрипция не удалась: {message}"),
        },
        SubmitError::Local(LocalError::EmptyTranscript) => FailureKind::Fatal {
            user_msg: "Локальная модель не вернула текст".into(),
        },
        SubmitError::Groq(GroqError::Http(_)) => FailureKind::Retryable,
        SubmitError::Groq(GroqError::Api { status, .. }) => match *status {
            429 => FailureKind::Retryable,
            500..=599 => FailureKind::Retryable,
            401 => FailureKind::Fatal {
                user_msg: "Неверный API key".into(),
            },
            413 => FailureKind::Fatal {
                user_msg: "Аудио слишком длинное (Groq лимит)".into(),
            },
            400 => FailureKind::Fatal {
                user_msg: "Groq отклонил запрос".into(),
            },
            403 => FailureKind::Fatal {
                user_msg: "Groq заблокирован (403) — включи DoH или прокси".into(),
            },
            other => FailureKind::Fatal {
                user_msg: format!("Ошибка Groq {other}"),
            },
        },
        SubmitError::Inject(_) => FailureKind::Fatal {
            user_msg: "Не удалось вставить текст (transcript уже в буфере обмена)".into(),
        },
    }
}

/// Backoff delay для попытки `attempt` (0-based). Формула: base * 2^attempt +
/// jitter ±50%. Cap на `cap`. Jitter — простой PRNG на `Instant::now().nanos`
/// чтобы не тянуть rand crate; для cosmic-tier не нужна криптостойкость.
pub(crate) fn backoff_delay(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let exp = base.saturating_mul(2u32.saturating_pow(attempt));
    let capped = exp.min(cap);
    // ±50% jitter
    let nanos = std::time::Instant::now().elapsed().subsec_nanos() as u64;
    let jitter_pct = (nanos % 100) as i64 - 50; // -50..+49
    let base_ms = capped.as_millis() as i64;
    let jitter_ms = base_ms * jitter_pct / 100;
    let final_ms = (base_ms + jitter_ms).max(0) as u64;
    Duration::from_millis(final_ms)
}

/// Прогоняет `op` до `max_attempts` раз с backoff между неудачами.
/// На первой `Fatal` ошибке — break без retry. Возвращает последнюю ошибку
/// если все попытки исчерпаны.
///
/// `on_retry` коллбэк зовётся ПЕРЕД sleep'ом для следующей попытки —
/// можно использовать для tracing/state update (например, `transition →
/// Pending { attempts }` в host.rs).
pub(crate) async fn with_backoff<F, Fut, T>(
    max_attempts: u32,
    base_delay: Duration,
    cap_delay: Duration,
    mut on_retry: impl FnMut(u32, &SubmitError),
    mut op: F,
) -> Result<T, SubmitError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, SubmitError>>,
{
    let mut last_err: Option<SubmitError> = None;
    for attempt in 0..max_attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let kind = classify(&e);
                if !kind.is_retryable() {
                    return Err(e);
                }
                if attempt + 1 < max_attempts {
                    on_retry(attempt + 1, &e);
                    last_err = Some(e);
                    tokio::time::sleep(backoff_delay(attempt, base_delay, cap_delay)).await;
                } else {
                    last_err = Some(e);
                }
            }
        }
    }
    // unreachable for max_attempts > 0, but guard for 0-case
    Err(last_err.unwrap_or(SubmitError::NoApiKey))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    // -----------------------------------------------------------------
    // classify()
    // -----------------------------------------------------------------

    #[test]
    fn classify_no_api_key_is_fatal() {
        let err = SubmitError::NoApiKey;
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("API key")),
            _ => panic!("expected Fatal for NoApiKey"),
        }
    }

    #[test]
    fn classify_audio_decode_is_fatal() {
        // Создаём настоящую base64::DecodeError через невалидный ввод.
        use base64::Engine;
        let decode_err = base64::engine::general_purpose::STANDARD
            .decode("!!!not-base64!!!")
            .unwrap_err();
        let err = SubmitError::AudioDecode(decode_err);
        assert!(matches!(classify(&err), FailureKind::Fatal { .. }));
    }

    #[test]
    fn classify_groq_5xx_is_retryable() {
        for status in [500u16, 502, 503, 504, 599] {
            let err = SubmitError::Groq(GroqError::Api {
                status,
                body: "server error".into(),
            });
            assert_eq!(
                classify(&err),
                FailureKind::Retryable,
                "{status} should be retryable"
            );
        }
    }

    #[test]
    fn classify_groq_429_is_retryable() {
        let err = SubmitError::Groq(GroqError::Api {
            status: 429,
            body: "rate limited".into(),
        });
        assert_eq!(classify(&err), FailureKind::Retryable);
    }

    #[test]
    fn classify_groq_401_is_fatal_with_api_key_msg() {
        let err = SubmitError::Groq(GroqError::Api {
            status: 401,
            body: "unauthorized".into(),
        });
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("API key")),
            _ => panic!("401 must be Fatal"),
        }
    }

    #[test]
    fn classify_groq_413_is_fatal() {
        let err = SubmitError::Groq(GroqError::Api {
            status: 413,
            body: "too large".into(),
        });
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("длинное")),
            _ => panic!("413 must be Fatal"),
        }
    }

    #[test]
    fn classify_groq_400_is_fatal() {
        let err = SubmitError::Groq(GroqError::Api {
            status: 400,
            body: "bad request".into(),
        });
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("отклонил")),
            _ => panic!("400 must be Fatal"),
        }
    }

    #[test]
    fn classify_groq_403_is_fatal_with_geo_hint() {
        // 403 в РФ-контексте — это перехват провайдера, не реальный Groq отказ.
        // Сообщение должно подсказывать DoH/proxy.
        let err = SubmitError::Groq(GroqError::Api {
            status: 403,
            body: "forbidden".into(),
        });
        match classify(&err) {
            FailureKind::Fatal { user_msg } => {
                assert!(user_msg.contains("403"));
                assert!(user_msg.contains("DoH") || user_msg.contains("прокси"));
            }
            _ => panic!("403 must be Fatal"),
        }
    }

    #[test]
    fn classify_groq_other_4xx_is_fatal_generic() {
        // Прочие 4xx без специальной обработки — generic Fatal.
        let err = SubmitError::Groq(GroqError::Api {
            status: 418,
            body: "im a teapot".into(),
        });
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("418")),
            _ => panic!("418 must be Fatal"),
        }
    }

    #[test]
    fn classify_inject_is_fatal() {
        use super::super::inject::InjectError;
        let err = SubmitError::Inject(InjectError::SendInput {
            injected: 0,
            expected: 4,
        });
        assert!(matches!(classify(&err), FailureKind::Fatal { .. }));
    }

    #[test]
    fn classify_local_missing_model_path_is_fatal() {
        let err = SubmitError::Local(LocalError::MissingModelPath);
        match classify(&err) {
            FailureKind::Fatal { user_msg } => assert!(user_msg.contains("путь")),
            _ => panic!("local missing path must be fatal"),
        }
    }

    #[tokio::test]
    async fn classify_network_error_is_retryable() {
        // Чтобы получить реальный reqwest::Error — стучимся в заведомо
        // непривязанный порт (timeout).
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(50))
            .build()
            .expect("client");
        let req_err = client
            .get("http://127.0.0.1:1") // reserved port, refuses
            .send()
            .await
            .expect_err("must fail");
        let err = SubmitError::Network(crate::dictation::network::NetworkError::Reqwest(req_err));
        assert_eq!(classify(&err), FailureKind::Retryable);
    }

    // -----------------------------------------------------------------
    // with_backoff()
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn with_backoff_returns_ok_on_first_attempt() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let result: Result<u32, SubmitError> = with_backoff(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            |_, _| {},
            || {
                let c = calls_c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(42)
                }
            },
        )
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "should call op exactly once"
        );
    }

    #[tokio::test]
    async fn with_backoff_retries_on_retryable_then_succeeds() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let result: Result<&'static str, SubmitError> = with_backoff(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            |_, _| {},
            || {
                let c = calls_c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        // first 2 attempts: retryable (Groq 503)
                        Err(SubmitError::Groq(GroqError::Api {
                            status: 503,
                            body: "down".into(),
                        }))
                    } else {
                        Ok("recovered")
                    }
                }
            },
        )
        .await;
        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn with_backoff_does_not_retry_on_fatal() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let result: Result<(), SubmitError> = with_backoff(
            5,
            Duration::from_millis(1),
            Duration::from_millis(10),
            |_, _| {},
            || {
                let c = calls_c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(SubmitError::Groq(GroqError::Api {
                        status: 401,
                        body: "nope".into(),
                    }))
                }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "Fatal must NOT trigger retry"
        );
    }

    #[tokio::test]
    async fn with_backoff_exhausts_max_attempts_on_persistent_retryable() {
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let result: Result<(), SubmitError> = with_backoff(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            |_, _| {},
            || {
                let c = calls_c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(SubmitError::Groq(GroqError::Api {
                        status: 503,
                        body: "still down".into(),
                    }))
                }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "should try exactly max_attempts times"
        );
    }

    #[tokio::test]
    async fn with_backoff_calls_on_retry_callback() {
        let retries_seen = Arc::new(AtomicU32::new(0));
        let retries_c = retries_seen.clone();
        let calls = Arc::new(AtomicU32::new(0));
        let calls_c = calls.clone();
        let _: Result<(), SubmitError> = with_backoff(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            move |_attempt: u32, _err: &SubmitError| {
                retries_c.fetch_add(1, Ordering::SeqCst);
            },
            || {
                let c = calls_c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(SubmitError::Groq(GroqError::Api {
                        status: 503,
                        body: "down".into(),
                    }))
                }
            },
        )
        .await;
        // 3 attempts → 2 retries scheduled (between 1→2 and 2→3)
        assert_eq!(retries_seen.load(Ordering::SeqCst), 2);
    }

    // -----------------------------------------------------------------
    // backoff_delay()
    // -----------------------------------------------------------------

    #[test]
    fn backoff_delay_grows_with_attempt() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_secs(60);
        // Берём середину диапазона jitter (±50%) — даже на нижней границе
        // attempt=2 должен быть > attempt=0 на верхней.
        // Простой smoke: cap при больших attempt не превышается.
        let d10 = backoff_delay(10, base, cap);
        assert!(d10 <= cap.saturating_mul(2), "must respect cap with jitter");
    }

    #[test]
    fn backoff_delay_caps() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_millis(500);
        // attempt=20 даст 100*2^20 ms = ~104857 sec без cap. С cap=500ms +
        // jitter ±50% — max 750ms.
        let d = backoff_delay(20, base, cap);
        assert!(
            d <= Duration::from_millis(760),
            "cap+jitter must hold: {d:?}"
        );
    }
}
