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

pub const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// Hardcoded prompt для Whisper. Короткий, с domain-терминами Mundus/Mundus
/// + явно указывает что это русская речь с пунктуацией. Не описывает задачу
///   («ты транскрибатор…») — Whisper это копирует в выход. Только пример стиля.
const HARDCODED_PROMPT: &str = concat!(
    "Привет! Это транскрипция русской речи с правильной пунктуацией — точками, ",
    "запятыми, тире, вопросительными и восклицательными знаками. В тексте могут ",
    "встречаться термины: API, Groq, Whisper, GPT, Anthropic, React, TypeScript. ",
    "Сохраняй естественные паузы и интонацию говорящего."
);

/// Пороги фильтрации сегментов от Whisper. Откалиброваны под docs OpenAI
/// (https://github.com/openai/whisper/discussions/1252 и др.).
const NO_SPEECH_PROB_THRESHOLD: f64 = 0.6;
const AVG_LOGPROB_THRESHOLD: f64 = -1.0;
const LONG_FORM_CHUNK_SECONDS: u32 = 30;

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
            s.no_speech_prob < NO_SPEECH_PROB_THRESHOLD && s.avg_logprob > AVG_LOGPROB_THRESHOLD
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
    endpoint: &str,
    api_key: &str,
    wav_bytes: Vec<u8>,
    language: &str,
    model: &str,
    prompt: &str,
) -> Result<TranscriptionResult, GroqError> {
    let chunks = split_wav_for_transcription(&wav_bytes);
    if chunks.len() > 1 {
        let mut parts: Vec<String> = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let part =
                transcribe_single_wav(client, endpoint, api_key, chunk, language, model, prompt)
                    .await?;
            let trimmed = part.text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_owned());
            }
        }
        return Ok(TranscriptionResult {
            text: parts.join(" ").trim().to_owned(),
        });
    }
    transcribe_single_wav(
        client, endpoint, api_key, wav_bytes, language, model, prompt,
    )
    .await
}

async fn transcribe_single_wav(
    client: &reqwest::Client,
    endpoint: &str,
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
        .post(endpoint)
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

#[derive(Debug)]
struct PcmWav<'a> {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    data: &'a [u8],
}

pub(crate) fn split_wav_for_transcription(wav_bytes: &[u8]) -> Vec<Vec<u8>> {
    let Some(parsed) = parse_pcm_wav(wav_bytes) else {
        return vec![wav_bytes.to_vec()];
    };
    let bytes_per_sample = usize::from(parsed.bits_per_sample / 8);
    let frame_bytes = usize::from(parsed.channels).saturating_mul(bytes_per_sample);
    let max_data_bytes = usize::try_from(parsed.sample_rate)
        .unwrap_or(0)
        .saturating_mul(usize::try_from(LONG_FORM_CHUNK_SECONDS).unwrap_or(0))
        .saturating_mul(frame_bytes);
    if frame_bytes == 0 || max_data_bytes == 0 || parsed.data.len() <= max_data_bytes {
        return vec![wav_bytes.to_vec()];
    }

    let chunk_data_bytes = max_data_bytes - (max_data_bytes % frame_bytes);
    if chunk_data_bytes == 0 {
        return vec![wav_bytes.to_vec()];
    }
    parsed
        .data
        .chunks(chunk_data_bytes)
        .map(|data| {
            encode_pcm_wav(
                data,
                parsed.sample_rate,
                parsed.channels,
                parsed.bits_per_sample,
            )
        })
        .collect()
}

fn parse_pcm_wav(bytes: &[u8]) -> Option<PcmWav<'_>> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut data: Option<&[u8]> = None;

    while offset.checked_add(8)? <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let start = offset + 8;
        let end = start.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        if id == b"fmt " {
            if size < 16 {
                return None;
            }
            let audio_format = u16::from_le_bytes(bytes[start..start + 2].try_into().ok()?);
            channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().ok()?);
            sample_rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().ok()?);
            bits_per_sample = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().ok()?);
            if audio_format != 1 || channels == 0 || bits_per_sample != 16 {
                return None;
            }
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }
        offset = end + (size % 2);
    }

    Some(PcmWav {
        sample_rate,
        channels,
        bits_per_sample,
        data: data?,
    })
}

fn encode_pcm_wav(data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let byte_rate = sample_rate
        .saturating_mul(u32::from(channels))
        .saturating_mul(u32::from(bits_per_sample / 8));
    let block_align = channels.saturating_mul(bits_per_sample / 8);
    let data_len = u32::try_from(data.len()).unwrap_or(u32::MAX);
    let riff_len = 36u32.saturating_add(data_len);
    let mut wav = Vec::with_capacity(44 + data.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(data);
    wav
}

#[cfg(test)]
#[path = "groq/tests.rs"]
mod tests;
