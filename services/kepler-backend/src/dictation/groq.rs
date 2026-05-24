// Groq Cloud STT client — POST audio/transcriptions endpoint
// (OpenAI-compatible). Whisper-large-v3.
//
// Docs: https://console.groq.com/docs/speech-to-text
//
// Multipart fields:
//   file              — WAV bytes (16kHz mono PCM)
//   model             — "whisper-large-v3"
//   response_format   — "verbose_json" (даёт segments для фильтрации галлюцинаций)
//   temperature       — "0" (детерминированный декодинг, см. блок ниже)
//   language          — ISO 639-1 hint (опционально, пропускаем для "auto")
//   prompt            — domain hint (≤ 224 токенов)
//
// Anti-hallucination политика (по убыванию важности):
//   1. language hint — без него Whisper «прыгает» на другой язык и фантазирует.
//   2. temperature=0 — отключает sampling, убирает повторы и YouTube-loop'ы.
//   3. response_format=verbose_json + segment filtering:
//        no_speech_prob > 0.6  → segment отбрасываем (фоновые тишины,
//                                 на которых Whisper выдаёт «Спасибо за
//                                 просмотр», «Подписывайтесь на канал» — это
//                                 артефакт обучения на YouTube).
//        avg_logprob   < -1.0  → segment отбрасываем (низкая уверенность).
//   4. Hardcoded короткий prompt (≤ 1-2 предложения) с domain-терминами.
//      Длинный prompt сам начинает галлюцинироваться в выход.

use serde::Deserialize;
use thiserror::Error;

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Hardcoded prompt для Whisper. Короткий, с domain-терминами Kosmos/Kepler
/// + явно указывает что это русская речь с пунктуацией. Не описывает задачу
/// («ты транскрибатор…») — Whisper это копирует в выход. Только пример стиля.
const HARDCODED_PROMPT: &str = "Привет! Это транскрипция русской речи с правильной пунктуацией — точками, запятыми, тире, вопросительными и восклицательными знаками. В тексте могут встречаться термины: API, Groq, Whisper, GPT, Anthropic, React, TypeScript. Сохраняй естественные паузы и интонацию говорящего.";

/// Пороги фильтрации сегментов от Whisper. Откалиброваны под docs OpenAI
/// (https://github.com/openai/whisper/discussions/1252 и др.).
const NO_SPEECH_PROB_THRESHOLD: f64 = 0.6;
const AVG_LOGPROB_THRESHOLD: f64 = -1.0;

#[derive(Debug, Deserialize, Default)]
pub struct VerboseSegment {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub no_speech_prob: f64,
    #[serde(default)]
    pub avg_logprob: f64,
}

#[derive(Debug, Deserialize)]
pub struct VerboseResponse {
    pub text: String,
    #[serde(default)]
    pub segments: Vec<VerboseSegment>,
}

#[derive(Debug)]
pub struct TranscriptionResult {
    pub text: String,
}

#[derive(Debug, Error)]
pub enum GroqError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Groq API {status}: {body}")]
    Api { status: u16, body: String },
}

/// Возвращает текст после фильтрации галлюцинаций по `no_speech_prob` и
/// `avg_logprob`. Если segments пустой (старый response_format / Groq не
/// прислал) — fallback на `resp.text`.
pub fn filter_segments(resp: &VerboseResponse) -> String {
    if resp.segments.is_empty() {
        return resp.text.clone();
    }
    let kept: Vec<&str> = resp
        .segments
        .iter()
        .filter(|s| {
            s.no_speech_prob < NO_SPEECH_PROB_THRESHOLD
                && s.avg_logprob > AVG_LOGPROB_THRESHOLD
        })
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect();
    // Если фильтр отсёк ВСЁ — лучше вернуть пустую строку (значит шумовая
    // запись без речи), а не raw text с галлюцинациями.
    kept.join(" ").trim().to_owned()
}

/// Отправляет WAV bytes в Groq. `language` "auto" → пропускаем поле (Whisper
/// сам определит). `prompt` пустой → подставляем `HARDCODED_PROMPT`.
/// Любой не-200 → `GroqError::Api` с телом для диагностики.
pub async fn transcribe(
    client: &reqwest::Client,
    api_key: &str,
    wav_bytes: Vec<u8>,
    language: &str,
    model: &str,
    prompt: &str,
) -> Result<TranscriptionResult, GroqError> {
    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let effective_prompt = if prompt.trim().is_empty() {
        HARDCODED_PROMPT
    } else {
        prompt
    };

    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_owned())
        .text("response_format", "verbose_json")
        .text("temperature", "0")
        .text("prompt", effective_prompt.to_owned());

    if language != "auto" && !language.is_empty() {
        form = form.text("language", language.to_owned());
    }

    let resp = client
        .post(GROQ_ENDPOINT)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(GroqError::Api {
            status: status.as_u16(),
            body,
        });
    }

    let parsed: VerboseResponse = resp.json().await?;
    let filtered = filter_segments(&parsed);
    Ok(TranscriptionResult { text: filtered })
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_segment(text: &str, no_speech: f64, avg_logprob: f64) -> VerboseSegment {
        VerboseSegment {
            text: text.into(),
            no_speech_prob: no_speech,
            avg_logprob,
        }
    }

    #[test]
    fn filter_keeps_high_confidence_segments() {
        let resp = VerboseResponse {
            text: "raw text not used".into(),
            segments: vec![
                make_segment("Привет.", 0.1, -0.3),
                make_segment("Как дела?", 0.05, -0.4),
            ],
        };
        let out = filter_segments(&resp);
        assert_eq!(out, "Привет. Как дела?");
    }

    #[test]
    fn filter_drops_high_no_speech_prob() {
        // Классическая Whisper-галлюцинация: на тишине выдаёт «Спасибо за просмотр»
        // с no_speech_prob ~ 0.95. Должно быть выкинуто.
        let resp = VerboseResponse {
            text: "shouldn't matter".into(),
            segments: vec![
                make_segment("Привет.", 0.1, -0.3),
                make_segment("Спасибо за просмотр!", 0.95, -0.5),
                make_segment("Подписывайтесь на канал.", 0.85, -0.6),
            ],
        };
        let out = filter_segments(&resp);
        assert_eq!(out, "Привет.");
    }

    #[test]
    fn filter_drops_low_logprob() {
        // avg_logprob ниже -1.0 — модель неуверенна, скорее всего галлюцинация.
        let resp = VerboseResponse {
            text: "ignored".into(),
            segments: vec![
                make_segment("Привет.", 0.1, -0.3),
                make_segment("какой-то шум", 0.2, -1.5),
            ],
        };
        let out = filter_segments(&resp);
        assert_eq!(out, "Привет.");
    }

    #[test]
    fn filter_returns_empty_when_all_segments_dropped() {
        // Шумовая запись без речи: все segments фильтруются → возвращаем пусто,
        // а не raw text (который содержал бы галлюцинации).
        let resp = VerboseResponse {
            text: "Спасибо за просмотр! Подписывайтесь!".into(),
            segments: vec![
                make_segment("Спасибо за просмотр!", 0.95, -0.5),
                make_segment("Подписывайтесь!", 0.92, -0.7),
            ],
        };
        let out = filter_segments(&resp);
        assert_eq!(out, "");
    }

    #[test]
    fn filter_falls_back_to_text_when_no_segments() {
        // Старый response_format / Groq не вернул segments → fallback.
        let resp = VerboseResponse {
            text: "fallback text".into(),
            segments: vec![],
        };
        let out = filter_segments(&resp);
        assert_eq!(out, "fallback text");
    }

    #[tokio::test]
    async fn returns_text_on_success() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/openai/v1/audio/transcriptions");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(
                        r#"{"text":"привет мир","segments":[{"text":"привет мир","no_speech_prob":0.05,"avg_logprob":-0.3}]}"#,
                    );
            })
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let part = reqwest::multipart::Part::bytes(b"fake".to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .expect("mime");
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3")
            .text("response_format", "verbose_json")
            .text("temperature", "0")
            .text("language", "ru");
        let resp = client
            .post(&url)
            .bearer_auth("fake-key")
            .multipart(form)
            .send()
            .await
            .expect("send");
        assert!(resp.status().is_success());
        let parsed: VerboseResponse = resp.json().await.expect("json");
        assert_eq!(filter_segments(&parsed), "привет мир");
    }

    #[tokio::test]
    async fn auto_language_skips_field() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/openai/v1/audio/transcriptions")
                    .matches(|req| {
                        let body = String::from_utf8_lossy(req.body.as_deref().unwrap_or(&[]));
                        !body.contains("name=\"language\"")
                    });
                then.status(200)
                    .body(r#"{"text":"ok","segments":[{"text":"ok","no_speech_prob":0.1,"avg_logprob":-0.5}]}"#);
            })
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/openai/v1/audio/transcriptions", server.base_url());
        let part = reqwest::multipart::Part::bytes(b"x".to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .expect("mime");
        let language = "auto";
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3")
            .text("response_format", "verbose_json")
            .text("temperature", "0");
        if language != "auto" && !language.is_empty() {
            form = form.text("language", language.to_owned());
        }
        let resp = client
            .post(&url)
            .bearer_auth("k")
            .multipart(form)
            .send()
            .await
            .expect("send");
        assert!(resp.status().is_success());
        mock.assert_async().await;
    }
}
