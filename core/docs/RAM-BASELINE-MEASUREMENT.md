# RAM Baseline Measurement

Статус: active
Дата: 2026-05-14
Связано: Phase 1 — миграция четырёх standalone-апок в Kepler ecosystem (один Electron host + Rust `kepler-backend` + `ark-core-rpc`).

## Цель

Результат измерения определяет, нужно ли двигаться в Phase 2 (extension architecture с aggressive pre-warming) или достаточно текущей standalone-модели.

## Подход

Сравниваем два состояния «pool of RAM», снятых при одинаковых условиях:

- `baseline` — четыре standalone-апки (старая модель). Запускаются вручную, ждут до полной загрузки UI.
- `kepler` — Kepler ecosystem (новая модель). `kepler-shell` (Electron host) + `kepler-backend` (Rust) + `ark-core-rpc` (Rust).

Метрики берём через `Get-Process`:

- `WorkingSet64` — резидентная память (physical RSS).
- `PrivateMemorySize64` — приватный commit (без shared pages).

Дочерние процессы Electron'а (`renderer`, `gpu-process`, `utility`) находим через `Get-CimInstance Win32_Process` по `ParentProcessId` и классифицируем по `CommandLine` (`--type=...`).

Snapshot снимается **один раз** после фиксированного warmup (по умолчанию 15 секунд) — этого достаточно, чтобы Electron инициализировал V8 / renderer / GPU, а Rust сервисы открыли DB и подписались на каналы.

## Как запустить

### Baseline

1. Закрой Kepler ecosystem (если запущен).
2. Запусти:

   ```powershell
   pwsh scripts/measure-kepler-ram.ps1 -Mode baseline
   ```

3. Скрипт ждёт 15 секунд warmup и сохраняет отчёт в `.tmp/ram-measurement-baseline-<timestamp>.json`.

### Kepler

1. Закрой все четыре standalone-апки.
2. Запусти Kepler ecosystem (`kepler-shell.exe`). Дождись загрузки.
3. Запусти:

   ```powershell
   pwsh scripts/measure-kepler-ram.ps1 -Mode kepler
   ```

### Compare

После двух запусков:

```powershell
pwsh scripts/measure-kepler-ram.ps1 -Compare
```

Читает самые свежие baseline и kepler файлы из `.tmp/` и печатает diff: `Экономия: X MB (P%)` либо `Регрессия: ...`.

## Параметры

- `-Mode <baseline|kepler>` — режим (по умолчанию `kepler`).
- `-Warmup <seconds>` — задержка перед snapshot (по умолчанию 15).
- `-Duration <seconds>` — зарезервировано на будущее (multi-sample). Сейчас один snapshot.
- `-OutFile <path>` — куда сохранить JSON. По умолчанию `.tmp/ram-measurement-<mode>-<timestamp>.json`.
- `-Compare` — режим сравнения двух последних отчётов.

## Формат отчёта

```json
{
  "mode": "kepler",
  "timestamp": "2026-05-14T...",
  "warmup_s": 15,
  "processes": [
    {
      "pid": 1234,
      "name": "kepler-shell",
      "role": "electron-main",
      "working_set_bytes": 145678336,
      "private_bytes": 89456128
    }
  ],
  "totals": {
    "working_set_mb": 412.3,
    "private_mb": 305.7
  }
}
```

## Интерпретация результатов

| Экономия (RSS)                  | Решение                                                                                                                              |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `< 100 MB`                      | Extension model не оправдывает усилий. Остаёмся в standalone-режиме, Phase 2 отменяется или откладывается.                           |
| `100..300 MB`                   | Соответствует ожиданиям review. Двигаемся в Phase 2 — extension architecture.                                                        |
| `> 300 MB`                      | Отлично, aggressive pre-warming (предзагрузка extensions при старте host'а) целесообразен.                                           |
| Регрессия (Kepler `>` baseline) | Что-то пошло не так в Phase 1. Перед Phase 2 обязательно профилировать `kepler-shell` (V8 heap, native modules, retained renderers). |

## Caveats

- **Idle vs. active.** Скрипт снимает peak в момент после warmup. Если внутри Eden открыт большой vault, в Delphi загружено много задач — RAM будет значительно выше, чем у «launched but idle» апок. Для честного сравнения **открой одинаковые данные** в обоих режимах.
- **Один snapshot, не average.** Это фотография, а не профилирование во времени. Для устойчивого числа сделай 3 запуска подряд (закрывай/запускай заново) и усредни.
- **Background activity.** Антивирус, бэкапы, индексаторы могут влиять на cold-start. Делай измерения на «спокойной» системе, без активных фоновых задач.
- **Working Set vs. Private Bytes.** WorkingSet включает shared pages (например, общая системная DLL'ка). Private bytes — точнее как «сколько эта программа реально хочет». В отчёте сохраняем оба, но в `-Compare` сравниваем WorkingSet (то, что user видит в Task Manager).
- **Electron child processes.** Скрипт находит детей через `Get-CimInstance Win32_Process` + `ParentProcessId`. Это снимок — между `Get-CimInstance` и `Get-Process` ребёнок мог умереть/родиться. Допустимая погрешность.
- **WMI на медленных машинах.** `Get-CimInstance Win32_Process` может занимать ~1-3 секунды. Это **не входит** в warmup — выполняется после.

## Связанные решения

- Phase 1 pivot к Kepler ecosystem зафиксирован в `dd.md` и `TODO.md`.
- Phase 2 (extensions) — open, решение принимается **по результатам** этого измерения.
