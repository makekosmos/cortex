# manager-gpui

GPUI-реализация Mundus Manager — панель управления Engine (KOS-130).
Покрывает разделы и операции через контракт `/v1/rpc` + `/v1/health` +
`/v1/info`.

## Запуск

```bash
cargo run --manifest-path manager-gpui/Cargo.toml
```

Приложение читает `engine.lock.json` из `MUNDUS_DATA_DIR` (по умолчанию
`~/.config/Mundus` на Linux, `%APPDATA%\Mundus` на Windows,
`~/Library/Application Support/Mundus` на macOS) на каждый запрос — перезапуск
Engine подхватывается без рестарта приложения.

## Engine

Manager работает только поверх живого Engine (`mundus-engine`). Перед созданием
окна он проверяет authenticated `/v1/health`: если Engine не готов, запускает
бинарник из установленного пакета и ждёт `status: ready`. Если бинарник не найден
или запуск не удался, окно Manager не открывается; на macOS показывается ошибка.
Для standalone/dev-сборки путь можно задать через `MUNDUS_ENGINE_PATH`.

Engine запускается независимо от UI: закрытие Manager и `Cmd+Q` завершают только
Manager, не Engine. На macOS добавлено нативное меню приложения с `Cmd+Q`.
`manager-gpui --check-engine` проверяет тот же startup-контракт без открытия окна.

Для разработки Engine можно запустить заранее:

```bash
MUNDUS_DATA_DIR=/tmp/mundus-dev cargo run -p engine
MUNDUS_DATA_DIR=/tmp/mundus-dev cargo run --manifest-path manager-gpui/Cargo.toml
```

Если Engine перестал отвечать после открытия Manager, внизу показывается
баннер с ошибкой и кнопкой «Обновить». Это runtime-сбой, а не разрешение открыть
Manager без готового Engine.

Данные страниц кешируются в памяти на время жизни UI: повторная навигация
использует готовые snapshots и не дублирует уже выполняющиеся запросы. `Cmd+R` (macOS, также меню «Вид → Обновить данные») / `Ctrl+R`
(Windows/Linux) принудительно перечитывает страницу; успешные mutations помечают
кешированные snapshots устаревшими, чтобы следующая навигация загрузила их заново.

Все страницы используют общий центрированный контейнер с максимальной шириной
760 px (включая виртуализированный список использования).

Текстовые кнопки всех страниц и модальных окон создаются через `src/button.rs`
(включая `widgets::btn` / `btn_id`). Подпись имеет явные 12.5 px / line-height
16 px, как компактные действия Zeron; внутренний GPUI Medium label иначе
использует 16 px независимо от размера текста страницы. Обёртка сохраняет
обычную геометрию Imago/GPUI (высота 32 px), цвета, focus, disabled и обработчики.
Accessible name задаётся отдельно от визуальной подписи. Не используйте прямые
Imago/GPUI button constructors в views: тест проверяет этот общий маршрут.

Синхронизация показывает отдельные блоки текущего и связанных устройств.
Имя текущего компьютера берётся из ОС один раз при запуске Manager; на macOS
используется `scutil --get ComputerName`. Та же реализация используется Engine
для advertised sync name (явный `MUNDUS_DEVICE_NAME` в Engine остаётся приоритетным).
Под именем отображается `ОС / версия Manager`, слева — embedded Hugeicons:
ноутбук для macOS, монитор для Windows, разные смартфоны для Android/iOS.
Старые peer snapshots пока не передают platform/version: UI не угадывает их
по имени, использует нейтральную иконку компьютера и сообщает об отсутствии
метаданных. Поля `platform` / `manager_version` уже поддерживаются отображением.
Ниже находится плашка «Добавить устройство»: форма подключения и копирование
своего кода раскрываются по кнопке. Engine возвращает pairing ticket строкой;
UI также понимает legacy-объекты с `ticket` / `code`. Пока раздел открыт,
состояния устройств обновляются в фоне раз в 3 секунды без сброса snapshots.
Названия страниц и кнопка обновления в titlebar не дублируются.

Статическая структура страницы не зависит от async slots: заголовки и подписи
известных полей рисуются сразу, а loading/error заменяют только значения.
Версия Mundus/Manager известна из сборки, не требует запроса и не меняется в
текущем процессе. Версия/API/сборка Engine приходят из `/v1/info` и используют
кеш навигации; проверка доступности на открытой странице «О приложении» идёт в
фоне раз в 5 секунд. Диагностический журнал и snapshot больше не запрашиваются
при открытии страницы. Инструменты поддержки раскрываются отдельно по кнопке.

## Покрытие (Vue → GPUI)

| Раздел | Операции |
| --- | --- |
| Данные | `manager.data.summary/types/list/search/storage` |
| Синхронизация | `get_sync_snapshot`, `get_own_iroh_ticket`, `connect_with_pairing_code`, `disconnect_peer` |
| Маркетплейс | `store.catalog/refresh`, `packages.list/install/set_enabled/uninstall/catalog_status/disclosure/refresh_catalog` |
| Движок | `engine.settings.get/set` (warm timeout, usage tracker) |
| Настройки | `manager.db_backups.list/create/validate/restore`, автозапуск (Host) |
| Интеграции | `integrations.list/set_credential/clear_credential/sync_now` |
| О приложении | `/v1/health`, `/v1/info`; инструменты поддержки и `manager.diagnostics.support_bundle.*` по явному действию |
| Обновления | `store.refresh`, `packages.refresh_catalog`, `packages.install` |
| Ключи | `dictation.get_config/verify_api_key/set_api_key/clear_api_key/test_connectivity/get_stats` |
| Браузер | `browser.json` `persistData` (тот же файл, что пишет Host) |
| Разработка | `packages.install_development`, `packages.uninstall` |

Host-возможности ушли в Engine/Manager: автостарт и апдейтер живут в Engine,
системные диалоги и crash reports — на уровне ОС/Engine.

## Разработка

```bash
cargo fmt --manifest-path manager-gpui/Cargo.toml
cargo clippy --manifest-path manager-gpui/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path manager-gpui/Cargo.toml
```

`MANAGER_GPUI_OFFSCREEN=1` паркует окно за пределами экрана (smoke-прогоны).
