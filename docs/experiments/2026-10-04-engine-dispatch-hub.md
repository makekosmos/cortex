# engine_dispatch hub + package_service extraction (KOS-338, 2026-10-04)

Сравнение с baseline KOS-330:
[`2026-10-04-engine-build-baseline.md`](./2026-10-04-engine-build-baseline.md)
и с предыдущим слайсом
[`2026-10-04-engine-packages-split.md`](./2026-10-04-engine-packages-split.md).
Та же машина, toolchain и методология (тёплый mbx, marker-fn в конец файла,
restore после замера, замеры — медиана стабильных прогонов в общем `target/`).

## Design choice

`engine_dispatch` сам по себе уже является trait-like boundary
(`DispatchHandler = Arc<dyn Fn(DispatchRequest) -> DispatchFuture>`), поэтому
отдельный `engine-dispatch-api` крейт не понадобился: hub уехал целиком в
`engine-base` вместе с `auth` (lease/token helpers нужны и `package_launch`,
и `ws_server`/`engine_api`). `#[cfg(test)]` observer/test-hooks переведены на
`cfg(any(test, feature = "test-support"))` — тот же механизм, что в
engine-dictation (KOS-334): engine test targets получают их через dev-dep
feature unification.

## Move map

`engine-base` += `engine_dispatch` (~560 LOC), `auth` (~255 LOC).

`engine-packages` += `package_service` (~8.5k), `native_apps` (~2k),
`catalog` (~0.7k), `package_registration` (~0.5k), `package_launch` (~0.5k),
`background_task` (~50). Фасады: `pub use engine_base::*` (auth +
engine_dispatch резолвятся автоматически) и расширенный
`pub use engine_packages::{…}` в `runtime/src/lib.rs` — все `crate::<module>`
пути в engine, ws_server, engine_api, agents, integrations, backend_tray и
ITs работают без правок.

In-crate shim `pub(crate) use engine_base::{…}` расширен: `auth`,
`engine_dispatch`, `lock_file`, `protocol_version`, `priority`.

Механические детали:
- `include_bytes!` пути в `native_apps/descriptor.rs` сдвинуты на `../../`.
- `package_service::tests` fixtures (используются engine_api/ws_server
  тестами) — `pub mod tests` под `cfg(any(test, feature="test-support"))`;
  `tempfile` переехал в `[dependencies]` (fixtures компилятся в non-test
  сборке с test-support).
- `#[cfg(test)]` хелперы, дёргаемые тестами engine (`LaunchLeaseRegistry::create/
  try_create_at/len/expire*`, `PackageService::open_for_test`) — на
  `any(test, feature="test-support")` (+ старый `package-worker-fixture`).
- `pub(crate)` → `pub` там, где engine потребляет через фасад (lease.rs,
  `register_package_definitions`, `background_task`, package_launch API).
- Единственная обратная ссылка `package_service → engine_api` была в тесте
  (`crate::engine_api::register_package_definitions` — thin wrapper);
  заменена прямым вызовом `service.register_package_definitions`.
- `native_apps` env-override для тестов: `#[cfg(not(test))]` →
  `#[cfg(not(any(test, feature = "test-support")))]` — engine-тесты должны
  видеть то же поведение, что и раньше.

## Transport decision (fork note, KOS-333)

Оба транспорта остаются: HTTP `engine_api` (`/v1/rpc`) и WS `ws_server`
собирают один и тот же `EngineDispatcher` поверх одного handler'а. Тикет не
удалял транспорты — оба живы, оба проходят parity-тест
`engine_api::tests::production_dispatcher_has_real_http_ws_socket_parity_and_owner_isolation`.

## Результаты, секунды

| Случай                                       | Baseline (этот прогон) | После split |
| -------------------------------------------- | ---------------------: | ----------: |
| incr-edit package_service/helpers.rs         |  27.5 → 5.7–5.9 steady |   7.1 steady¹ |
| incr-edit engine_api/handlers/request.rs     |                 ~5.9   |   5.1–7.5   |
| `cargo check -p engine --all-targets` (+fixtures) | n/a                |   ~12.6     |
| `nextest -p engine-base -p engine-packages`  |                    n/a |  286 pass   |
| `nextest -p engine`                          |                    n/a |  409 pass   |

¹ Первые прогоны после git mv дороже (mbx прогревает инкрементальный индекс
нового юнита): 39 s → 21.6 s → 7.1 s steady. Правка в package_service теперь
пересобирает lib engine-packages (~0.9×engine по объёму раньше — теперь
это отдельный крейт) + relink engine; чистый фронтенд engine lib при этом
не пересобирается.

## Что осталось в top crate (engine)

`engine_api`, `ws_server`, `agents`, `manager_api`, `integrations`,
`backend_tray`, `installer`, `updater`, `engine_supervisor`, `focus`,
`pomodoro*`, `sync`, `app_network`, `main`. Следующие кандидаты по тому же
паттерну: `engine_api`+`ws_server` (оба уже на EngineDispatcher), `agents`,
`integrations`.

## FOLLOW-UP

- `background_task`/`package_launch` стали `pub` через фасад (были
  `pub(crate) mod`) — API surface engine-packages можно стянуть обратно,
  если потребуется, через `#[doc(hidden)]` или перенос потребителей.
- `pub(crate)` → `pub` применён механически по moved-модулям; точное
  минимальное множество не вычислялось.
