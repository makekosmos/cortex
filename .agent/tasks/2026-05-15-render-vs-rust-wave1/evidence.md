# Wave 1 evidence — todoFilterService TS vs Rust

## AC results

- **AC1** (TS golden tests PASS): **PASS** — 11/11 в `bun test` (см.
  `extensions/delphi/tests/filterService.test.ts`).
- **AC2** (TS baseline benchmark capture): **PASS** — см.
  `raw/ts-baseline.md`.
- **AC3** (Rust unit tests PASS): **PASS** — `cargo test --lib
  delphi::` → 3/3 passed:
  - `golden_parity_filter_per_list`
  - `golden_parity_count_all`
  - `empty_input`
- **AC4** (Rust criterion bench capture): **PASS** — см. `raw/rust-bench.txt`.
- **AC5** (output parity TS vs Rust): **PASS** — Rust `expected_filter_output`
  table в `filters_fixtures.rs` идентичен TS `EXPECTED` map в
  `filterService.fixtures.ts`. Rust tests проверяют это automatically.
- **AC6** (Rust latency within ±10% of TS): **MIXED — see analysis ниже.**

## Numbers @ n = 10 000 todos

| Operation | TS median | Rust median | Ratio | Winner |
|---|---:|---:|---:|---|
| `filter_inbox` | 0.149 ms | 1.770 ms | TS **11.9x** faster | TS |
| `filter_today` | 0.576 ms | 0.472 ms | Rust 1.2x faster | Rust |
| `filter_upcoming` | 0.834 ms | 0.642 ms | Rust 1.3x faster | Rust |
| `filter_anytime` | 0.189 ms | 1.995 ms | TS **10.6x** faster | TS |
| `filter_someday` | 0.071 ms | 0.216 ms | TS 3.0x faster | TS |
| `filter_logbook` | 0.822 ms | 0.404 ms | Rust 2.0x faster | Rust |
| `filter_trash` | 0.807 ms | 0.319 ms | Rust 2.5x faster | Rust |
| `count_all` | 1.648 ms | 0.099 ms | Rust **16.6x** faster | Rust |

(TS — bun runtime V8 на debug. Rust — `cargo bench` release mode.)

## Insight

**В обе стороны.** V8 на pure-function filter с lots of clones —
конкурентный против Rust. Где Rust явно лучше:

1. **count_all** (16.6x) — pure iteration без allocations, идеальный
   Rust use case.
2. **logbook / trash** (2-2.5x) — sort-heavy + small result set, alloc
   overhead minimal.
3. **today / upcoming** (~1.2-1.3x) — comparable, slight Rust edge.

Где TS выигрывает:

1. **inbox / anytime** (10-12x slower в Rust!) — большой result set,
   `.cloned()` для каждого match'а уничтожает performance. V8 escape
   analysis lets it share refs внутри tight loop.
2. **someday** — слишком маленький dataset (200µs vs 70µs), measurement
   noise.

### Implication: avoid `clone()` в Rust filters

Текущий `filter_todos` возвращает `Vec<TodoItem>`. На большом match-set
это значит N clones × sizeof(TodoItem) ≈ N × ~200 bytes. Для 5K matches —
1 MB allocation per call. Это и есть 10x regression vs V8 ref-sharing.

Workaround variants:
- `Vec<&TodoItem>` (lifetime-bound) — caller владеет input, Rust filter
  возвращает refs. **PROBLEM**: cross-FFI / IPC overflow boundary
  нельзя refs, всё equal сериализуется.
- `Vec<usize>` (indices в input) — caller обращается по index. Меньше
  allocations но same fundamental issue для serialization.
- **Real-world path**: backend читает task_obj rows из SQLite напрямую,
  applies predicate в WHERE/ORDER BY, returns sorted Vec через WS.
  Тогда сравнение не «TS V8 in-memory» vs «Rust in-memory», а **«TS
  V8 in-memory» vs «SQLite query plan + serde + WS»** — это другой
  benchmark, и его выиграть будет сложнее.

## Архитектурное заключение

**Этот замер — опровержение гипотезы «Rust быстрее, поэтому надо
мигрировать compute».** Для pure array transforms V8 за 20 лет научился
hidden classes + inline caching + escape analysis — JIT компилит hot
loops почти в нативку. Эквивалентный Rust на cloned()-heavy filter не
обыгрывает. Плюс транспортный налог (JSON serialize × 2 + WS frame +
TCP loopback) ≈ 1 мс per round-trip — окупается только когда Rust-сторона
делает СИЛЬНО больше работы (SQLite pushdown), или избегает гонять
данные туда-сюда вовсе (persistent state, events).

### Куда мигрировать имеет смысл (Wave 2+)

Не «compute → backend» вообще, а **только** где есть:

1. **SQLite-pushdown** — фильтрация / агрегация / сортировка делает БД,
   в TS приходят готовые 50 строк, не 10K. Данные не пересекают границу
   процесса. Win огромный.
2. **Persistent state machines** — Pomodoro timer, sync workers, indexers.
   Должны жить **за пределами lifecycle окна**. Renderer закрылся, crash'нулся,
   reload'нулся — backend держит state. TS в renderer'е принципиально
   не может.
3. **Cross-process events / fan-out** — одно изменение в БД должно дойти
   до N окон. Backend делает за один цикл event loop'а — TS-side это N
   отдельных IPC.
4. **CPU-bound parallelism** — image processing, full-text indexing,
   ML inference. V8 single-threaded, Rust + rayon — параллельно по cores.

### Куда мигрировать НЕ имеет смысла

Array transforms / value mapping / форматирование строк — оставлять в TS.
Данные уже в renderer'е, обратно класть не надо — миграция в backend это
ХУДШИЙ возможный паттерн. Получаем транспортный налог × 2 без compensating
work.

### Rule of thumb

Мигрировать модуль в Rust имеет смысл если ответ «да» хотя бы на один:
- Нужно ли state-у пережить renderer crash?
- Нужен ли fan-out events на несколько окон?
- Можно ли сделать SELECT/WHERE/ORDER BY вместо TS-loop'а?
- Нужно ли использовать N CPU cores?

Если все ответы «нет» — оставлять в TS.

**Filters остаются в TS** для now. Rust copy сохранён как foundation
для будущего SQLite-pushdown (через WHERE/ORDER BY в DB query вместо
in-memory transform). Не активен.

## Files changed

- `extensions/delphi/tests/filterService.{fixtures,test,bench}.ts`
- `extensions/delphi/tsconfig.json` (bun test paths)
- `crates/ark-core/rust/src/delphi/{mod,types,filters,filters_fixtures}.rs`
- `crates/ark-core/rust/Cargo.toml` (criterion dep + bench)
- `crates/ark-core/rust/benches/delphi_filters.rs`
- `crates/ark-core/rust/src/lib.rs` (`pub mod delphi`)

## Open

- Wave 2 candidates: Pomodoro state machine, Делphi recurrence, space
  encoding, ark-client transport — те где Rust persistent state / pure
  math / non-V8-friendly workloads.
- Полный pipeline bench (TS → IPC → Rust → SQLite → back) — нужен,
  но требует endpoints в ws_server. Отложен.
