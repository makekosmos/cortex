# Wave 1 — Render vs Rust migration (TDD)

## Контекст

После 5-агентного аудита (см. предыдущая сессия) выделены candidates для
перевода JS/TS compute → Rust backend. Wave 1 = низкорисковые pure
functions, которые сейчас работают в Vue/TS и которые легко проверить
parity'ом.

## Workflow (proof loop strict)

1. **Baseline**: для текущего TS implementation — написать golden tests
   (input → expected output на fixtures) + benchmark (throughput / latency
   на synthetic data).
2. **Run + capture**: запустить, сохранить numbers в `evidence.md`.
3. **Port to Rust**: написать Rust version с **тем же контрактом**
   (signature, semantics). Unit tests на той же fixture data в Rust.
4. **Parity**: новый integration test — Vue/TS вызывает backend op,
   результат идентичен TS-version output для тех же inputs.
5. **Bench Rust**: criterion bench на той же synthetic data. Capture
   numbers, сравнить с TS baseline.
6. **Switch**: shim/store вызывает backend op вместо TS service.
   Existing app tests должны остаться зелёными.
7. **Evidence**: PASS/FAIL per AC.

## Wave 1 candidates (по приоритету)

### W1.1 — Делphi filters (`todoFilterService.ts`)

- **Source**: 132 LOC TS, 7 predicates + 5 sorters + 3 public funcs.
- **Pure**: нет I/O, нет async, чистые functions от TodoItem[]→TodoItem[].
- **Rust target**: `crates/ark-core/rust/src/delphi/filters.rs`. ARK op
  `tasks.list_by_smart_list(view: string) → Vec<ArkObjectRecord>`. Внутри
  query `SELECT * FROM objects WHERE type_id='task_obj' AND deleted_at
  IS NULL` затем apply predicate/sort на Rust-side.

**AC W1.1:**
- AC1: TS golden tests PASS (16+ test cases covering all 7 SmartList views,
  edge cases: empty, all-trashed, scheduled-today vs scheduled-other).
- AC2: TS benchmark выдаёт baseline (median ms for 10K todos × 7 lists =
  70K predicate calls + 5 sort passes).
- AC3: Rust unit tests PASS на тех же fixtures (json import от TS).
- AC4: Rust criterion bench дёргает ту же synthetic data. Captured number.
- AC5: Output parity — Rust result identical TS result для random 1K
  todos.
- AC6: TS bench → Rust bench: latency либо ниже либо в пределах ±10%.
  Если ниже — отлично; если выше — investigate (но не блокер если в
  пределах).

### W1.2 — Делphi recurrence (`services/recurrence/recurrence.ts`)

- 158 LOC. Pure math, date arithmetic. Daily/Weekly/Monthly/Yearly.
- Defer to W2 (после W1.1 pattern proven).

### W1.3 — Дashboard pickPrimary (`shell/src/dashboard/store.ts`)

- ~25 LOC. Trivial. Defer.

### W1.4 — Делphi space-manager Crockford32 encoding

- 284 LOC. Pure math. Defer.

### W1.5 — Arrancador stats aggregation

- ~30 LOC. Trivial. Defer.

## Out of scope

- W1.1 fully *replacing* TS filter — этап 6 «switch» можно отложить.
  Достаточно доказать parity (AC5) + bench (AC6).
- Делphi store mutations / Pomodoro state machine — W2+/W3+. High risk.

## Critical invariants

- Не trogать существующие Playwright e2e — все 22/22 должны остаться PASS.
- Backend changes — additive (новая op, не breaking change).
- TS code остаётся работоспособен (Делphi extension не падает).
