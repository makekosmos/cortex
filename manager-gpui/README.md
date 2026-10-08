# manager-gpui

GPUI-реализация Mundus Manager — панель управления Engine (KOS-130).
Покрывает разделы и операции через контракт `/v1/rpc` + `/v1/health` +
`/v1/info`.

## Запуск

Native release-сборка Engine и Manager с единой продуктовой версией из release
метаданных: `pnpm run build:native` в корне Cortex. `pnpm run build:native --dev`
собирает оптимизированный dev-пакет; обычный Cargo без метаданных тоже помечается
`(dev)`, а не выдаётся за production. Сборка не устанавливает приложение и не
включает автозапуск. Согласованная организация страниц: [NAVIGATION_PROPOSAL.md](NAVIGATION_PROPOSAL.md).

```bash
pnpm run dev    # из корня Cortex: собирает Engine и запускает Manager (общий MUNDUS_DATA_DIR)
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

Для разработки Engine можно запустить заранее (`pnpm run dev -- --engine-only`
или вручную):

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
760 px (включая виртуализированный список использования). Общие primitives
`src/page_layout.rs` задают inset 16/12 px, внутренние gaps 12 px, колонку
подписей 180 px, базовые строки названия/описания 18/16 px и 32 px поля ввода.
Смысловые группы отделены 32 px, обычные соседние карточки — 24 px; заголовок
связан со своей карточкой интервалом 12 px. Empty states не
добавляют второй горизонтальный отступ внутри карточки; control rows сохраняют
правый край и вертикальный центр при длинном имени.

Sidebar: Приложения / Данные / Активность; Девайсы / Интеграции / Ключи;
Настройки / Внешний вид / Обновления / О приложении. Приложения открываются на
вкладке «Установленные». Бэкапы находятся в «Данных». «Ключи» — отдельная
страница компаний; сохранение сейчас есть только у Groq. «Интеграции» остаются
источниками данных.
Настройки объединяют запуск Engine без UI, производительность и приватность. Разработка/FPS/локальные пакеты раскрываются явно и не загружаются
при закрытой панели. Пути хранения и API прежних операций не менялись.

«Внешний вид» — отдельная страница. Контролы режимов, выбора темы и шрифта
перенесены из Zeron UI (reference `1f7b74a7`), а не переосмыслены: превью
148 px с подписью снизу без внешней карточки, две строки выбора палитры с
dropdown 218 px / меню 260 px и трёхполосным образцом цвета, поисковый popup
шрифта 220 px и рядом dropdown размера 128 px. Высота triggers 32 px, radius
8 px; меню radius 12 px. Исходные стили и keyboard/outside-click поведения
адаптированы к GPUI Manager; данные по-прежнему принадлежат Engine.
[MIT notice Zeron](third_party/zeron-ui-LICENSE) и атрибуция Solar Icons
поставляются вместе с UI. Палитры и шрифты Zeron не копируются: используются существующие Imago и
установленные шрифты. Также доступны акцент темы/свой HEX/обои, нативные материалы,
установленные шрифты и размер 11–18 px (включая 12.5). Engine сохраняет
настройки на устройстве и отдаёт capabilities; UI не угадывает поддержку по ОС.
Manager применяет общий стиль всегда, Agenda — только при «Единый стиль
приложений». Остальным клиентам нужно подключить
[`appearance.get`](../runtime/APPEARANCE_CONTRACT.md).
Сбой чтения сохраняет последний стиль. Типографика масштабируется относительно
13 px через `src/theme.rs`, не меняя физические отступы, rem-сетку и hit targets.
Нативный frosted/blur работает через прозрачный Root и тонированный фон окна.
В Zeron Match берёт цвет обоев внутри приложения; здесь macOS Engine читает
картинку рабочего стола. При отсутствии доступа остаётся акцент темы.

Текстовые кнопки всех страниц и модальных окон создаются через `src/button.rs`
(включая `widgets::btn` / `btn_id`). Базовая подпись имеет 12.5 px / line-height
16 px и масштабируется с пользовательским шрифтом, как компактные действия Zeron; внутренний GPUI Medium label иначе
использует 16 px независимо от размера текста страницы. Обёртка сохраняет
обычную геометрию Imago/GPUI (высота 32 px), цвета, focus, disabled и обработчики.
Accessible name задаётся отдельно от визуальной подписи. Не используйте прямые
Imago/GPUI button constructors в views: тест проверяет этот общий маршрут.

Раздел «Девайсы» показывает отдельные блоки текущего и связанных устройств.
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

Обновления используют `currentVersion` / `channel` / `newVersion` из Engine,
не версию Cargo и не локальную догадку об ОС. `/v1/info` тоже отдаёт версию и
канал. Суффикс `(dev)` добавляется только при отображении, не к semver в API.
Engine возвращает `canInstall` и `installUnavailableReason`; текущий установщик
Windows-only. Другие платформы могут узнать о новой версии продукта, но не
скачивают Windows installer и не объявляют его доступным для автоустановки.

## Покрытие (Vue → GPUI)

| Раздел | Операции |
| --- | --- |
| Данные | `manager.data.summary/types/list/search/storage` |
| Девайсы | `get_sync_snapshot`, `show_pairing_code`, `connect_with_pairing_code`, `disconnect_peer`, `system.privileged.status/enable` |
| Маркетплейс | `store.catalog/refresh`, `packages.list/install/set_enabled/uninstall/disclosure/refresh_catalog` |
| Движок | `engine.settings.get/set` (usage tracker) |
| Настройки | `manager.db_backups.list/create/validate/restore`, автозапуск (Host) |
| Интеграции | `integrations.list/set_credential/clear_credential/sync_now` |
| О приложении | `/v1/health`, `/v1/info`; инструменты поддержки и `manager.diagnostics.support_bundle.*` по явному действию |
| Обновления | `store.refresh`, `packages.refresh_catalog`, `packages.install` |
| Ключи | `dictation.get_config/verify_api_key/set_api_key/test_connectivity/get_stats` |
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
