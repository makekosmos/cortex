# Замеры после выделения engine-dictation (KOS-334, 2026-10-04)

Сравнение с baseline KOS-330:
[`2026-10-04-engine-build-baseline.md`](./2026-10-04-engine-build-baseline.md).
Та же машина, toolchain и методология (отдельный `CARGO_TARGET_DIR`,
тёплый mbx, marker-fn в конец файла, `git restore` после замера).

## Что изменилось

`runtime/src/dictation/` (~15.2k LOC) → `runtime/crates/engine-dictation`.
Фасад `pub use engine_dictation as dictation;` в `runtime/src/lib.rs` —
все `crate::dictation::*` пути в engine работают без правок. Внутри крейта
`pub(crate) use engine_base::{brand, data_dir, file_hash, process_tree};`
сохраняет прежние `crate::*` пути.

Диктаторные deps переехали с кодом: `transcribe-rs`(+`ort` load-dynamic,
KOS-345 сохранён), `arboard`, `hickory-resolver`, `libloading`.
`keyring`/`base64`/`reqwest`/`zip`/`flate2`/`tar` — в обоих манифестах
(shared), `hpke` — не dictation, остался в engine. Windows-фичи крейта —
только используемые (UI Input, WindowsAndMessaging, Media Audio, Com,
Foundation, Storage FileSystem).

## Результаты, секунды

| Случай                                          | Baseline | После split |
| ----------------------------------------------- | -------: | ----------: |
| clean `cargo build -p engine` (пустой target)   |       42 |      44.9 |
| incr-edit в dictation (стабильный прогон)       |       10 |       7.8 |
| incr-edit в ws_server (стабильный прогон)       |       10 |       8.7 |
| no-op rebuild                                   |     9–10 |       9.4 |
| `cargo test --no-run` после правки в dictation  |       66 |       7.5¹ |
| `cargo test -p engine-dictation` (203 теста)    |      n/a |     1.1 прогон / 7.5 test-build |

¹ `cargo test -p engine-dictation --no-run` — билдится только крейт, не
весь engine. Билд engine test-harness после правки в dictation больше
не нужен вообще, если менять только dictation.

Изоляция подтверждена логом компиляции: правка в dictation пересобирает
только `engine-dictation` + релинк bin (engine lib не трогается); правка в
`ws_server` пересобирает только `engine` (engine-dictation не трогается).

## Выводы для эпика KOS-329

1. Цикл «правка в dictation → сборка» — 7.8 с против 10 с (-22%); цикл
   «правка → test-build» — 7.5 с против 66 с (-89%). Это главный выигрыш:
   итерация на dictation больше не пересобирает 60k+ LOC engine lib.
2. Издержки на раскол минимальны: ~30 строк манифестов + фасад; clean build
   не деградировал (44.9 с vs 42 с — в пределах шума mbx).
3. `tests/it/dictation_worker_contract_windows.rs` остаётся в engine — он
   тестирует worker-contract supervisor'а через синтетический executor, а
   не dictation-библиотеку.
4. **Рекомендация: продолжать сплиты (KOS-335+)** по той же схеме —
   крейт → фасад → `test-support` feature для тестовых конструкторов.
