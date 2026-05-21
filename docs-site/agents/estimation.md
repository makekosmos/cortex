# Оценка времени — правила для агента

::: tip Цель
Я систематически переоцениваю / недооцениваю scope задач. Без feedback-loop'а это не самоисправляется. Эта страница — про **обязательную калибровку** при каждой оценке времени.
:::

## Когда применять

**Trigger** — я собираюсь назвать пользователю срок:

- «займёт примерно N часов / дней / недель»
- Перед заморозкой `spec.md` в новом `.agent/tasks/<DATE>-<slug>/`
- При архитектурном решении («это будет долго / коротко»)
- При прикидке roadmap-фазы

**Не применять** для:

- Опечаток, локальных переименований, edit-level правок
- Ответов на вопросы без работы (объяснения кода, проверка факта)

## Skill: `estimate-calibration`

Skill живёт в `~/.claude/skills/estimate-calibration/` (user-level, не в репо). Содержит:

- `SKILL.md` — инструкция, какие шаги делать.
- `log.jsonl` — NDJSON append-only / rewrite-on-close с записями prediction vs actual.
- `calibration.md` — периодический sweep (раз в 5-10 записей) с трендами.

Каждая запись:

```json
{
  "task_id": "<YYYY-MM-DD-slug>",
  "scope": "<one-line>",
  "estimate_h": 4,
  "started_at": "<ISO>",
  "completed_at": null,
  "actual_h": null,
  "variance_pct": null,
  "tags": ["vue-extension", "migration", "m"],
  "notes": "anchors: <prior task_ids>"
}
```

`estimate_h` / `actual_h` — active work time (часы взаимодействия), не календарные.

## Процедура

### Шаг 1 — read log

Прочитать `~/.claude/skills/estimate-calibration/log.jsonl`. Найти 3-5 наиболее похожих записей по `tags` / ключевым словам в `scope`. Посчитать средний `variance_pct`.

Если систематически переоцениваю на +40% — gut estimate × 0.6. Если недооцениваю на −30% — × 1.3.

### Шаг 2 — записать prediction (до начала работы)

Append к log.jsonl:

```json
{"task_id":"<slug>","scope":"<one-line>","estimate_h":<N>,"started_at":"<now UTC ISO>","completed_at":null,"actual_h":null,"variance_pct":null,"tags":[...],"notes":"anchors: <referenced task_ids>"}
```

### Шаг 3 — сообщить оценку пользователю с anchor

Не просто «4h», а:

> По gut intuition — 4h. Скорректировано на калибровку: похожие vue-extension migration'ы в прошлом стабильно занимали +60% к моей оценке (см. `2026-05-17-eden-extension`, `2026-04-25-arrancador-port`). Финально ставлю **6.5h**.

Если log пустой — отметить «no calibration history» и использовать гут.

### Шаг 4 — закрыть запись после завершения

Когда задача формально закрыта (proof loop: все AC PASS):

1. Найти row с тем же task_id и `completed_at: null`.
2. Update (rewrite файла, чтобы остаться append-only по таскам):
   - `completed_at`: ISO now
   - `actual_h`: wall-clock между started_at и completed_at, в часах
   - `variance_pct`: `((actual - estimate) / estimate) * 100`
   - `notes`: что обусловило вариацию (если значимая)

### Шаг 5 — sweep раз в 5-10 записей

Короткая ретроспектива в `~/.claude/skills/estimate-calibration/calibration.md`:

```markdown
## Sweep 2026-MM-DD (N records)

- Средний variance: +X%
- Хорошо оцениваю: <теги>
- Плохо оцениваю: <теги>, особенно <причина>
- Anchors с наименьшей вариацией: <task_ids>
```

## Anti-patterns

- ❌ Оценить **после** написания `spec.md` «потому что и так понятно». Калибровка не работает без честного gut estimate перед погружением.
- ❌ Подстраивать `actual_h` под `estimate_h` чтобы variance выглядел меньше. Записываю честно, в т.ч. embarrassingly большие отклонения (−91% Eden migration).
- ❌ Удалять записи с большим промахом. Они самые ценные.
- ❌ Считать только «прямой код» в `actual_h`. Включать: debugging, reverify, обновление docs, evidence сборку.
- ❌ Пропустить skill, потому что «эта задача проста». Skill — про калибровку, не про размер.

## Связанные документы

- [Proof loop](/concepts/proof-loop) — где `task_id` живёт.
- [Testing](./testing) — как валидировать что задача реально done.
- [Docs maintenance](./docs-maintenance) — оценка времени на doc updates тоже нужна.
