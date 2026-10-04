# Baseline сборки engine перед делением на крейты (KOS-330, 2026-10-04)

Цель: зафиксировать числа ДО crate-split эпика (KOS-329), чтобы последующие
слайсы могли сравниваться. Замеряется только `cargo build -p engine`
(debug-профиль), не весь workspace.

## Условия

- Машина: Linux VM (Debian 13, kernel 6.12.94+), Intel Xeon, 8 vCPU, 15 ГБ.
- Toolchain: stable rustc/cargo 1.95.0. Линковка через lld — `ld.lld` на PATH и
  `-fuse-ld=lld` в `.cargo/config.toml` (KOS-328).
- cargo идёт через shim mbx 1.15.0 с прогретым кэшем (сценарий «рабочая машина»).
- Код: `origin/main` @ `460a0059`, без локальных изменений.
- `profile.dev`: `debug = "line-tables-only"`, у зависимостей debuginfo off.
- Замер: отдельный `CARGO_TARGET_DIR=target-baseline` (не трогаем общий
  `target/`), секундомер вокруг вызова cargo. `cargo fetch` не входил.
- Инкрементальные правки: добавление `#[allow(dead_code)] fn kos330_bench_marker() {}`
  в конец файла, пересборка, `git restore` после замера.

## Результаты, секунды

| Случай                                                        | Прогон 1 | Прогон 2 |
| ------------------------------------------------------------- | -------: | -------: |
| clean `cargo build -p engine` (пустой target, тёплый mbx)     |       42 |        — |
| incr-edit `runtime/src/dictation/stats.rs`                    |    50 ¹ |       10 |
| incr-edit `runtime/src/package_service/helpers.rs`            |       12 |       10 |
| incr-edit `runtime/src/ws_server/lifecycle.rs`                |       23 |       10 |
| no-op rebuild                                                 |       22 |      9–37 |
| `cargo test -p engine --no-run` после правки в dictation      |       66 |        — |
| `cargo build -p engine --timings` (второй пустой target dir)  |      135 |        — |

¹ Первый инкрементальный прогон в свежей папке — mbx ещё заполнял индекс
target dir; дальше все три incr-случая стабильно ~10 с. No-op — 9–10 с при
тёплом индексе, всплески до ~37 с на фоновой работе mbx.

## --timings (135 с, 542 юнита)

Отчёт: [`2026-10-04-engine-build-baseline-cargo-timing.html`](./2026-10-04-engine-build-baseline-cargo-timing.html).

- Самый дорогой юнит — **engine lib: 35.6 с** (frontend 29.6 с, codegen 5.9 с).
  Начинается на 94-й секунде, после почти всех зависимостей.
- `mundus-engine` bin (линковка lld): 5.5 с, хвост сборки до 135.0 с.
- Дорогие зависимости: `ring` build-script 25.7 с, `ark-core` 19.4 с,
  `syn` ×2 12.4+11.4 с, `iroh` 8.9 с, `netlink-packet-route` 8.5 с.

## Выводы для crate-split

1. На тёплой машине цикл «правка → сборка» — ~10 с, и большая его часть —
   перекомпиляция lib-крейта engine (~10 с при инкрементальном кодогене против
   35.6 с с нуля) плюс линковка 5.5 с. Деление engine на крейты должно резать
   именно эти ~10 с.
2. `cargo test --no-run` после правки — 66 с: тестовая сборка заново компилит
   engine (lib + тестовые харнессы), что грубо удваивает работу.
3. mbx добавляет заметный постоянный overhead (no-op ~9–10 с с всплесками);
   при сравнении «до/после» мерить медианой нескольких прогонов.

## Как повторить

Скрипт: `/home/box/devin-runs/kos-330/bench.sh`. Суть:

```bash
export CARGO_TARGET_DIR=target-baseline
time cargo build -p engine                                    # clean
printf '\n#[allow(dead_code)]\nfn kos330_bench_marker() {}\n' \
  >> runtime/src/dictation/stats.rs
time cargo build -p engine                                    # incr
git restore runtime/src/dictation/stats.rs
time cargo build -p engine                                    # no-op
time cargo test -p engine --no-run                            # после правки
CARGO_TARGET_DIR=target-timings cargo build -p engine --timings
```
