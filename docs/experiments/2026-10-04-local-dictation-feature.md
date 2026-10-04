# Замер: фича `local-dictation` (KOS-337, 2026-10-04)

Цель: локальный dictation backend (`transcribe-rs` + `ort`/ONNX) не должен
собираться в дефолтных dev/agent билдах. Фича `engine/local-dictation`
включена в полном гейте и release; по умолчанию выключена.

## Условия

- Машина: Linux worktree `/workspace/wt/cortex-kos-337`, mbx shared cache
  прогрет (cache hits реплеят компиляции — холодный замер невозможен без
  `cargo clean`, что запрещено AGENTS.md).
- База: `kos-337` поверх origin/main @ 487746da.

## Размер зависимостей (`cargo tree --edges normal`, уникальных крейтов)

| Крейт | без фичи | с фичей | Δ |
|-------|----------|---------|---|
| `engine-dictation` | 239 | 273 | +34 |
| `engine` (весь бинарь) | 494 | 525 | +31 |

Δ — `transcribe-rs`, `ort`, `ort-sys`, `ndarray`, `rustfft`, `hound`,
`derive_builder`, `matrixmultiply`, `num-complex` и их транзитивные.

## Тайминги (теплый mbx/инкрементальный кэш)

| Замер | без фичи | с фичей |
|-------|----------|---------|
| Incr edit `local/backend.rs` → `cargo build -p engine-dictation` | 4.05 s | 4.25 s |
| `cargo build -p engine --bin mundus-engine` (первый прогон конфигурации) | 42.3 s | 26.6 s¹ |

¹ Порядок прогонов разный (off шёл первым и грел общие зависимости);
честный сигнал — размер дерева зависимостей (−31 крейт без фичи) и то, что
`transcribe-rs`/`ort` вообще не появляются в `cargo check -p engine-dictation`
без фичи.

## Поведение без фичи

- `local::transcribe`, `local::preload_server`,
  `transcribe_with_whisper_backend{,_model}`,
  `preload_with_whisper_backend{,_model}` →
  `LocalError::NotBuiltWithLocalDictation` ("local dictation backend not
  built with local-dictation feature"), классифицируется как Fatal (не
  ретраится). Groq/cloud не затронут.
- `local/tests.rs` не компилируется (чистый skip); контракт ошибки покрыт
  `local/tests_disabled.rs` (4 теста).
- Parakeet ONNX path (`run_parakeet`, `ensure_onnxruntime`, `ORT_DYLIB_NAME`)
  выcfg'ан целиком; `ort` остаётся load-dynamic (KOS-345) когда фича включена.
