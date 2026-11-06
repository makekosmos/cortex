use super::{
    encode_pcm_wav, filter_segments, parse_pcm_wav, split_wav_for_transcription, transcribe,
    VerboseResponse, VerboseSegment,
};
use httpmock::prelude::*;

fn make_segment(text: &str, no_speech: f64, avg_logprob: f64) -> VerboseSegment {
    VerboseSegment {
        text: text.into(),
        no_speech_prob: no_speech,
        avg_logprob,
    }
}

fn make_pcm_wav_seconds(seconds: u32) -> Vec<u8> {
    let sample_rate = 16_000u32;
    let data_len = usize::try_from(sample_rate)
        .unwrap()
        .saturating_mul(usize::try_from(seconds).unwrap())
        .saturating_mul(2);
    let data = vec![0u8; data_len];
    encode_pcm_wav(&data, sample_rate, 1, 16)
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

#[test]
fn split_wav_for_transcription_keeps_short_audio_single_request() {
    let wav = make_pcm_wav_seconds(30);
    let chunks = split_wav_for_transcription(&wav);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], wav);
}

#[test]
fn split_wav_for_transcription_chunks_long_audio_by_30_seconds() {
    // Regression: 2026-06-04. Long-form Whisper/Groq path must not send
    // >30s dictation as one request, because users lose speech after the
    // first model window.
    let wav = make_pcm_wav_seconds(61);
    let chunks = split_wav_for_transcription(&wav);
    assert_eq!(chunks.len(), 3);
    let durations: Vec<usize> = chunks
        .iter()
        .map(|chunk| parse_pcm_wav(chunk).unwrap().data.len() / (16_000 * 2))
        .collect();
    assert_eq!(durations, vec![30, 30, 1]);
}

#[tokio::test]
async fn returns_text_on_success() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/openai/v1/audio/transcriptions");
            then.status(200)
                .header("content-type", "application/json")
                .body(concat!(
                    concat!(
                        r#"{"text":"привет мир","segments":[{"text":"привет мир","#,
                        r#""no_speech_prob":0.05,"#,
                    ),
                    r#""avg_logprob":-0.3}]}"#,
                ));
        })
        .await;

    let client = engine_base::http::client();
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
                .is_true(|req| {
                    let body = String::from_utf8_lossy(req.body_ref());
                    !body.contains("name=\"language\"")
                });
            then.status(200).body(concat!(
                r#"{"text":"ok","segments":[{"text":"ok","no_speech_prob":0.1,"#,
                r#""avg_logprob":-0.5}]}"#
            ));
        })
        .await;

    let client = engine_base::http::client();
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

#[tokio::test]
async fn transcribe_posts_long_audio_in_ordered_chunks() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST).path("/openai/v1/audio/transcriptions");
            then.status(200)
                .header("content-type", "application/json")
                .body(concat!(
                    r#"{"text":"часть","segments":[{"text":"часть","no_speech_prob":0.05,"#,
                    r#""avg_logprob":-0.3}]}"#
                ));
        })
        .await;

    let client = engine_base::http::client();
    let url = format!("{}/openai/v1/audio/transcriptions", server.base_url());
    let out = transcribe(
        &client,
        &url,
        "fake-key",
        make_pcm_wav_seconds(61),
        "ru",
        "whisper-large-v3",
        "",
    )
    .await
    .expect("transcribe");

    assert_eq!(out.text, "часть часть часть");
    assert_eq!(mock.calls_async().await, 3);
}
