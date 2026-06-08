# 2026-06-08 — Launcher file search runaway

## Цель

Остановить бесконечный polling `file_index.search` из скрытого launcher'а и закрыть дорогой backend path для коротких file-search запросов.

## Контекст

`LauncherView.vue` запускал файловый поиск при изменении query, а после успешного поиска сам планировал следующий запуск через 800 мс. Окно launcher'а скрывается, а не уничтожается, поэтому `onUnmounted()` не чистит таймер. Для коротких query backend уходил в `LIKE '%q%'` по `files.name/path`, что на большом индексе может держать CPU.

## Скоуп

В скоупе:

- Убрать бесконечный refresh file search из renderer.
- Добавить renderer-событие hide и чистить active search state при скрытии launcher'а.
- Не выполнять substring LIKE-scan для query короче 3 символов.
- Вернуть launcher `backgroundThrottling: true` там, где нет macOS occlusion workaround.
- Добавить regression-тесты/проверки на указанные инварианты.

Не в скоупе:

- Новый prefix-index для коротких запросов.
- Полная end-to-end диагностика backend counter за 60 секунд на реальном индексе пользователя.
- Рефакторинг большого `LauncherView.vue` на composable/component split.

## Acceptance Criteria

**AC1.** При скрытии launcher renderer инвалидирует текущий file-search run, очищает pending timer и очищает `fileCommands`; main process отправляет `kepler:window:hide` через preload bridge.

**AC2.** File search запускается только от изменения query: в `LauncherView.vue` нет self-rescheduling polling после успешного поиска, и для 10 быстрых изменений query невозможно получить бесконечный хвост вызовов после debounce.

**AC3.** Query длиной 1-2 символа не вызывает backend substring `LIKE '%q%'` scan по таблице `files`; backend возвращает пустой результат для таких запросов.

**AC4.** Launcher window не отключает background throttling на Windows/Linux. Исключение допускается только для macOS workaround против occlusion throttling.

**AC5.** Substantial/task guards выполнены или явно зафиксирована причина, почему конкретная проверка не была запущена.
