# Repo Hygiene Cleanup

## Goal

Привести репозиторий к более чистой monorepo-конфигурации: оставить один канонический Bun lockfile в корне, убрать из git явные generated/test artifacts и удалить tracked исторические хвосты, которые не относятся к активному продукту.

## Scope

- Root `.gitignore`
- Root `package.json`
- Удаление лишних tracked lockfile
- Удаление tracked `test-results` artifact
- Удаление tracked исторических данных в `apps/arrancador`
- Локальная синхронизация `AGENTS.md`, если текущая карта файлов меняется

## Out of Scope

- Физическое удаление всех локальных `node_modules/` из рабочего дерева
- Изменение Rust `Cargo.lock` политики
- Любые feature-изменения приложений

## Acceptance Criteria

- AC1: Root workspace остаётся канонической Bun entrypoint-конфигурацией, а [bun.lock](D:/Personal/Hobby/Coding/kosmos/bun.lock) остаётся единственным Bun lockfile в git.
- AC2: Явные tracked artifacts и test-output мусор удалены из git, включая `test-results/.last-run.json`.
- AC3: Root `.gitignore` закрывает найденные пробелы для `test-results/` и `.bun_tmp/`.
- AC4: Исторические tracked хвосты в `apps/arrancador`, не относящиеся к активному runtime, удалены из git.
- AC5: Затронутые `AGENTS.md` не противоречат новому состоянию дерева.
- AC6: Финальная проверка `git status --short` не показывает неожиданных generated/artifact файлов сверх осознанного cleanup diff.

## Verification Plan

- Проверить `git diff --stat`
- Проверить `git ls-files` для удалённых lockfile/artifact путей
- Проверить `git status --short`
- Проверить `package.json` и `.gitignore`
