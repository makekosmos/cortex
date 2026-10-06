// Retry с exponential backoff + классификация ошибок на retryable/fatal.
//
// Зачем: при network drop (DNS, TCP reset, 5xx, 429) — повторяем с jitter.
// При 401/400/413/audio decode — fail-fast, retry бесполезен.
//
// См. spec `.agent/tasks/2026-05-25-dictation-resilience/spec.md` § retry.rs.

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

/// Классификация `SubmitError` на основе типа и (для Groq Api) HTTP-статуса.
///
/// Правила:
/// - `AudioDecode` / `Inject*` → Fatal (caller bug / unrecoverable).
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
        SubmitError::Local(LocalError::SidecarUnavailable(_)) => FailureKind::Retryable,
        SubmitError::Local(LocalError::NotBuiltWithLocalDictation) => FailureKind::Fatal {
            user_msg: "Локальная диктовка недоступна в этой сборке".into(),
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
            user_msg: "Не удалось вставить текст — диктовка осталась в очереди".into(),
        },
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use std::time::Duration;

    // -----------------------------------------------------------------
    // classify()
    // -----------------------------------------------------------------

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
        let client = engine_base::http::client_builder()
            .timeout(Duration::from_millis(50))
            .build()
            .expect("client");
        let req_err = client
            .get("http://127.0.0.1:1") // reserved port, refuses
            .send()
            .await
            .expect_err("must fail");
        let err = SubmitError::Network(crate::network::NetworkError::Reqwest(req_err));
        assert_eq!(classify(&err), FailureKind::Retryable);
    }
}
