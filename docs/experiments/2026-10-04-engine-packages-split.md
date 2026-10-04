# Замеры после выделения engine-packages (KOS-336, 2026-10-04)

Сравнение с baseline KOS-330:
[`2026-10-04-engine-build-baseline.md`](./2026-10-04-engine-build-baseline.md).
Та же машина, toolchain и методология (тёплый mbx, marker-fn в конец файла,
restore после замера). Замер в общем `target/` на прогретом дереве — baseline
использовал отдельный `CARGO_TARGET_DIR`, поэтому абсолютные числа сравниваем
со «стабильными» прогонами baseline (~10 s incr / 66 s test-build).

## Что изменилось

`runtime/src/{package_store, package_worker_process, package_worker_broker,
package_worker_supervisor, package_worker_protocol, runtime_grants,
grant_authority}` (~14k LOC) → `runtime/crates/engine-packages`. Вместе с
ними уехали `ark_host` (~360 LOC, конкретный `ArkRequestExecutor`
supervisor'а) и `package_worker_secrets` (~300 LOC, `zeroize_secret` и
реестр хэндлов нужны и supervisor'у, и package_service — фасад сохраняет
путь).

- Фасад `pub use engine_packages::{…}` в `runtime/src/lib.rs` — все
  `crate::<module>::*` пути в engine и ITs (`tests/it/package_worker_*`,
  `phase5_runtime_grants`) работают без правок.
- In-crate shim `pub(crate) use engine_base::{…}` + `engine_dictation as
  dictation` сохраняет `crate::*` пути внутри перенесённого кода.
- `ManagerState` остаётся в `manager_api` (engine): supervisor получил
  feature-трейт `AutostartControl` (`set_autostart`), реализованный для
  `ManagerState` на стороне engine; `EngineCapabilityExecutor::new` принимает
  `Arc<dyn AutostartControl>`.
- `package-worker-fixture` проброшен: `engine/package-worker-fixture` →
  `engine-packages/package-worker-fixture` (+ прежний engine-base).
  Release `compile_error` живёт в `engine/src/lib.rs` и продолжает ловить
  feature на границе.
- `test-support` feature пробрасывает `engine-dictation/test-support`
  (`DictationHost::new_for_test` в тестах engine_capability).
- `pub(crate)` → `pub` там, где engine потреблял через фасад:
  `package_store::{is_reserved_name, normalize_path, MAX_*}`,
  `package_worker_secrets::zeroize_secret`,
  `GrantAuthorityRegistry::with_directory_root`.
- `package_service` НЕ тронут (coupled к native_apps/catalog/engine_dispatch —
  отдельный тикет после решения по hub).

## Результаты, секунды

| Случай                                             | Baseline | После split |
| -------------------------------------------------- | -------: | ----------: |
| incr-edit в package_worker_supervisor (стабильный) |   ~10 ¹ |    6.8–7.2 |
| incr-edit в package_worker_broker (стабильный)     |   ~10 ¹ |    6.9–7.2 |
| `cargo test --no-run` после правки в supervisor    |    66 ² |      10.0 ³ |
| engine lib + ITs (`nextest -p engine`)             |      n/a | 612 pass   |
| `nextest -p engine-packages` (113 тестов)          |      n/a | 4.2 прогон |

¹ Любая правка в engine lib в baseline стоила ~10 s (60k+ LOC крейт).
² `cargo test -p engine --no-run` после правки в dictation — пересборка
всего engine test-harness'а.
³ `cargo test -p engine-packages --no-run` — билдится только новый крейт.

Изоляция по логу mbx: правка в supervisor/broker пересобирает только
`engine-packages` + релинк (`3 incremental`); engine lib не трогается.

## Выводы для эпика KOS-329

1. Цикл «правка в worker stack → сборка» — ~7 s против ~10 s (-30%);
   «правка → test-build» — ~10 s против 66 s (-85%).
2. Главная сложность сплита — не объём, а обратные зависимости:
   `ArkHost` (конкретный executor) и `ManagerState` (autostart) тянули
   supervisor обратно в engine. Решено переносом ark_host + трейтом
   `AutostartControl` без изменения публичных путей.
3. Рекомендация та же: продолжать сплиты; для `package_service` сначала
   решить вопрос hub'а (native_apps/catalog/engine_dispatch).
