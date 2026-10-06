# AGENTS.md — cortex

Cortex — это Mundus Engine (`runtime/`), встроенное ядро данных ARK (`core/`),
Manager (`manager-gpui/`) и упаковка/релизы (`desktop/`). Агенда, Memoria и
Dictation живут в своих репозиториях (`makekosmos/agenda-gpui`,
`makekosmos/memoria-gpui`, `makekosmos/dictation`) и ставятся Engine из GitHub
Releases. Главная платформа — Windows; macOS собирается и проверяется в CI,
Linux годится для разработки Engine (`docs/linux-dev.md`).

## Карта

- `runtime/` — крейт `engine` (бинарь `mundus-engine`): Engine API
  (`/v1/rpc`, WebSocket), хост и супервизор пакетов, `focus.*`/`pomodoro.*`,
  диктовка, апдейтер, трей. Подкрейты: `runtime/crates/{engine-base,
  engine-dictation,engine-packages,engine-indexes,package-protocol,
  pe-version-info}`. ARK работает внутри процесса: `runtime/crates/engine-packages/src/ark_host.rs`.
- `core/crates/ark-core/` — Rust + SQLite, контракт и инварианты в
  `core/crates/ark-core/AGENTS.md` и `docs/{ark-core,sync,write-boundary}.md`.
- `manager-gpui/` — GPUI-приложение Manager. Это отдельный Cargo workspace со
  своим `Cargo.lock`, корневой `cargo`-команды его не затрагивают.
- `desktop/` — скрипты установщика (NSIS), релизный BOM и публикация.
- `scripts/` — локальный гейт (`check-*.mjs`), `dev.mjs`; `docs/` — решения и
  замеры (`docs/experiments/` — журнал измерений, не правила).

## Границы

- Приложения не ходят в ОС и в базу сами: микрофон, хоткеи, вставка текста,
  блокировка фокуса, ключи API и запись в SQLite принадлежат Engine
  (`docs/repo-split-decisions.md`, `docs/write-boundary.md`). Новая
  возможность для приложения — это новая операция Engine, а не обход.

## Запуск

```text
pnpm run dev                     # собрать Engine и запустить Manager на нём
pnpm run dev -- --engine-only    # только Engine
pnpm run dev -- --data-dir DIR   # общий MUNDUS_DATA_DIR (по умолчанию <tmp>/mundus-dev)
```

Manager запускает Engine сам, но уже живой Engine в той же папке данных
переиспользуется как есть, поэтому после правок в `runtime/` останови старый
(его `pid` лежит в `<data-dir>/engine.lock.json`). Версии Rust и Node берутся
из `toolchain.json`; `rust-toolchain.toml` из него генерируется
(`node scripts/check-toolchain.mjs --write`).

## Проверки

CI на GitHub запускается на каждый PR и на push в `main` и является
источником истины. Локальный гейт — не копия CI: часть проверок (см. ниже)
идёт только локально.

- `.github/workflows/ci.yml`: job `lint` (Ubuntu: rustfmt обоих workspace,
  actionlint, `cargo deny check bans`, `check-dep-pins`, версии тулчейна;
  advisories не блокируют) и job `check` на Windows и macOS (clippy с
  `-D warnings`, сборка, тесты Engine через nextest, clippy/тесты/сборка
  Manager, нативные хелперы macOS). `installer-smoke.yml` собирает и реально
  запускает установщик (на PR — при правках установщика, и перед ночным
  релизом); `nightly-release.yml` публикует релиз в 03:00 UTC.
- Только локально (в CI их нет, обход хуков никто не поймает): `check:brand`,
  `check:source-size`, `check:test-skips`, `check:layout`, oxlint, oxfmt,
  `check:core-pin`, `test:static`, сборка рантайма (`runtime-staging`).
  Не обходи хуки.
- Workflows можно и нужно править, когда они расходятся с реальностью;
  `actionlint` в CI проверяет синтаксис, actions закреплены по SHA.
- `pnpm run check` — полный локальный гейт (список в
  `scripts/check-plan-commands.mjs`); `pnpm run check:affected` — только по
  изменённым файлам (планировщик при сомнении выбирает полный прогон).
  Результат кешируется по хэшу дерева: после прогона не правь файлы до push.
- Хуки ставит `pnpm install` (`pnpm run prepare`): pre-commit гоняет только
  быстрые проверки, pre-push — план по diff. Не обходи их через `--no-verify`.
- Rust-тесты идут через `cargo nextest` (нужна версия из
  `.config/nextest.toml`), по процессу на тест, утечка дочернего процесса — это
  падение. Тесты не должны писать в пользовательский `%TEMP%`: раннер
  выдаёт каждому прогону свой временный каталог.
- Узкий цикл правка → проверка: `node scripts/quick-check.mjs` (крейты по
  `git status`; `--check-only`, `--files …`, фильтр nextest позиционно) или
  `cargo check -p <crate> --all-targets` и `cargo nextest run -p <crate>`.
  Для `engine` добавляй
  `--features package-worker-fixture,markdown-bridge-fixture`. Manager:
  `pnpm run check:manager-gpui`. Полный гейт обязателен перед push.
- Локальная диктовка (whisper.cpp, Parakeet/ONNX) собирается только с фичей
  `engine/local-dictation`: её включает `pnpm run clippy`, а CI и обычные
  сборки нет. Правя `runtime/crates/engine-dictation/src/local/`, проверяй с
  фичей (`docs/onnxruntime.md`, `docs/experiments/2026-10-04-local-dictation-feature.md`).
- Линкер и флаги кодогенерации держи в `.cargo/config.toml`, не в
  `RUSTFLAGS`: тогда кеш сборки общий у всех рабочих копий.

## Правила кода

- Мёртвый код удаляй сразу, вместе с тестами, которые только его и проверяют.
  Не прячь его через `#[allow(dead_code)]`, `-A` в командной строке или
  «пока не подключено»: история остаётся в git. Исключений в clippy нет — чини
  код. Единственный источник политики линтов: `[workspace.lints]` в корневом
  `Cargo.toml` (`unwrap_used` и `unreachable` — deny; `panic`, `todo`,
  `unimplemented` — warn) плюс `cargo clippy -- -D warnings`. Точечный
  `#[allow(clippy::…)]` на элементе допустим только с причиной рядом.
  В `manager-gpui` таблицы `[lints]` нет, там действует один `-D warnings`.
- Размер файла: больше 300 строк — предупреждение, больше 500 — ошибка
  (`scripts/check-source-size.mjs`). Список `GRANDFATHERED` только сокращается.
- Нельзя `#[ignore]` без причины и тест, который «проходит» ранним `return`
  при провалившейся подготовке (`scripts/check-test-skips.mjs`).
- Имя продукта — Mundus. Упоминания `kosmos`/`kepler` допустимы только с
  пометкой `MIGRATION(KOS-267)` или в `scripts/brand-allowlist.json`
  (`pnpm run check:brand`, `docs/brand-legacy-identifiers.md`).
- Зависимости: общие версии заданы один раз в `[workspace.dependencies]`;
  точные и pre-release пины записаны в `ALLOWED` в
  `scripts/check-dep-pins.mjs`; новые дубликаты версий ловит `deny.toml`
  (убрал дубликат — удали его `skip` в том же PR). `Cargo.lock` коммитится,
  CI собирает с `--locked`. Версии `gpui`/`gpui-component`/`gpui-base` в
  Manager должны совпадать с агендой, Memoria и Dictation: две версии gpui в
  одной сборке — ошибка типов, поднимай их вместе.
- Схема данных ARK меняется только добавлением, без разрушающих миграций;
  любая запись в синхронизируемые данные обновляет версию синхронизации
  (`core/crates/ark-core/AGENTS.md`).

## Релиз

Версия и BOM выводятся из коммита, руками их не пишут; `main` релизит ночной
workflow. Не меняй версию, не создавай теги и не публикуй релизы без прямой
просьбы. Подробности — в `README.md` и `desktop/DEV.md`.
