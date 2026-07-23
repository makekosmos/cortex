# Problems

## AC4 — FAIL after user verification

Пользователь уточнил, что распределение Codewars должно быть именно chart/gauge, а не `CoderBreakdown` с горизонтальными полосами. На реальном dev-экране панель также осталась пустой: renderer был пересобран, но запущенный `target/debug/kepler-backend.exe` оставался старым и не выполнял новый rank backfill.

Продолжение вынесено в `2026-07-19-coder-codewars-kyu-chart`: переиспользовать gauge для Codewars и проверить собранный backend, не объявляя PASS только по mock UI.
