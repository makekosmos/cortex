# Coder Codewars Kyu Chart

## Goal

Показывать на вкладке Codewars настоящий gauge/chart распределения завершённых kata по kyu и устранить пустое состояние после backfill.

## Scope

- Переиспользовать существующий semicircle gauge для LeetCode и Codewars без дублирующего chart-компонента.
- Для Codewars отрисовать сегменты и количественную легенду `8 kyu` → `1 kyu`.
- Убрать ошибочно добавленную breakdown-карточку уровней.
- Собрать изменённый dev-backend и проверить rank backfill path; не менять schema, RPC contract или ARK write boundary.

## Acceptance Criteria

**AC1.** На вкладке Codewars виден semicircle chart с центральным числом ranked kata и отдельными сегментами для присутствующих kyu.

**AC2.** Легенда Codewars показывает уровни в порядке `8 kyu` → `1 kyu` и точные количества; отдельной breakdown-карточки «Решённые kata по kyu» нет.

**AC3.** Существующие Codewars objects без `rank` по-прежнему запускают полный backfill для текущего username, а dev-backend с этим кодом успешно собирается.

**AC4.** LeetCode gauge сохраняет прежние Easy / Medium / Hard значения, крупное центральное число и отсутствие видимых подписей «Уровень алгоритмов» / «Решено».

**AC5.** Проходят targeted Rust/TypeScript tests, desktop typecheck/build, ARK guard, Electron E2E и визуальная проверка обеих вкладок.
