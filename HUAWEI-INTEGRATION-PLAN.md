# Huawei Health integration plan

## Статус

Отложенный план. Реализация не начата.

Цель — локальная интеграция Huawei Health для личного использования без Health Kit API и
без периодического ручного экспорта данных. Источник — данные, которые официальное Android-
приложение Huawei Health загружает в своё private app-data после открытия соответствующих дней.

## Требуемый пользовательский сценарий

1. В Kosmos пользователь открывает интеграцию Huawei Health.
2. Kosmos предлагает установить локальный Huawei Bridge.
3. Bridge устанавливает и настраивает выбранный Android-эмулятор.
4. Только при первичной настройке пользователь видит окно эмулятора и самостоятельно проходит
   Huawei ID login, CAPTCHA и 2FA. Kosmos не получает пароль.
5. После входа эмулятор запускается невидимо, Huawei Health автоматически прогружает нужные дни,
   Bridge извлекает появившиеся локальные данные и импортирует их в ARK.
6. Первый импорт обходит всю доступную историю с checkpoint. Последующие импорты начинают с
   последней успешной синхронизации и повторно проверяют небольшой overlap последних дней.

## Зафиксированные выводы

### Официальный API не рассматривается

Health Kit API, verification/scopes и отдельный Android companion не являются transport этой
интеграции. Целевой сценарий уже был практически подтверждён пользователем через Huawei Health в
эмуляторе: после открытия дня приложение загружало данные в локальное app-data.

### ABI Huawei Health

17 июля 2026 года был скачан текущий официальный Huawei Health APK по ссылке Huawei:

`https://appgallery.cloud.huawei.com/appdl/C10414141`

Результат проверки архива:

- размер APK: 203 913 390 байт (194,4 МБ);
- native ABI: `arm64-v8a` и `armeabi-v7a`;
- `x86` и `x86_64` отсутствуют.

Временный APK после проверки удалён.

Следствие: штатный x86_64 Android Emulator обладает лучшим настоящим headless-режимом, но не
подходит как основной кандидат. Запуск ARM-сборки через полную cross-architecture эмуляцию будет
слишком медленным. Нужен Windows-эмулятор с аппаратно ускоренной ARM binary translation.

### Требования к эмулятору

Обязательные:

- Huawei Health ARM APK действительно запускается и проходит Huawei ID login;
- ADB доступен локально и не публикуется в LAN;
- можно получить root-доступ к `/data/user/0/com.huawei.health`;
- VM продолжает выполнять UI/network работу после скрытия окна;
- окно можно не показывать при обычной синхронизации;
- доступно программное управление lifecycle: launch, readiness, stop;
- UI можно автоматизировать через accessibility/UIAutomator либо стабильный fallback;
- состояние авторизации и app-data сохраняется между запусками;
- приемлемые disk/RAM/CPU и быстрый warm start;
- эмулятор можно установить и настроить из UX Kosmos без Android Studio.

Желательные:

- отдельный каталог VM под Kosmos;
- snapshot/quick boot;
- динамический диск с возможностью compact;
- конфигурация 2 CPU и 1,5–2 ГБ RAM;
- отключение звука, камер, GPS, игровых сервисов, рекламы и лишнего launcher UI;
- управляемое обновление без потери Huawei-сессии;
- удаление Bridge отдельно от уже импортированных ARK-данных.

## Shortlist

### 1. MuMu Player 12

Плюсы:

- ARM translation;
- включаемый root;
- официальный ADB workflow;
- стандартный endpoint `127.0.0.1:7555`;
- через ADB документированы install/uninstall, simulated input, push/pull, screenshots;
- подходит для управления без взаимодействия с gaming launcher после настройки.

Минусы и неизвестные:

- нет подтверждённого настоящего headless-флага;
- вероятно, окно придётся скрывать через Win32 `ShowWindow(SW_HIDE)`;
- нужно доказать, что hidden window не ставит renderer/network на паузу;
- proprietary gaming runtime может содержать лишние службы и рекламу;
- root может обнаруживаться Huawei Health.

Документация:

- `https://www.mumuplayer.com/help/win/developers-essentials-manual.html`
- `https://www.mumuplayer.com/help/win/emulator-function-shortcuts.html`

### 2. LDPlayer 9

Плюсы:

- ARM translation;
- root и локальный ADB;
- официальный CLI `ldconsole.exe`/`dnconsole.exe` для launch/quit/install/run/ADB;
- Android 9 потенциально меньше современного игрового Android image и при этом удовлетворяет
  заявленному Huawei минимуму Android 8.

Минусы и неизвестные:

- нет подтверждённого настоящего headless-флага;
- window hiding придётся делать через Win32;
- gaming services и launcher нужно измерить и отключить;
- root может ломать запуск Huawei Health;
- нужно проверить, насколько стабилен CLI актуального LDPlayer 9, поскольку часть публичной
  документации описывает более старые версии.

Документация:

- `https://www.ldplayer.net/blog/introduction-to-ldplayer-command-line-interface.html`
- `https://www.ldplayer.net/blog/introduction-to-version-4.0.37-and-3.102-features.html`

### Отсеянные варианты

#### Android Emulator

Сильные стороны: документированный `-no-window`, `-no-audio`, Quick Boot/snapshots, отдельный
`userdata-qemu.img`, чистый CLI и минимальная интеграция без игрового launcher.

Причина отказа: Huawei Health APK не содержит x86/x86_64. Полная ARM-эмуляция на x64 Windows не
соответствует требованиям скорости. К варианту можно вернуться, только если появится доказанно
быстрая и легально поставляемая ARM translation для официального AVD.

Документация:

- `https://developer.android.com/studio/run/emulator-commandline`
- `https://developer.android.com/studio/run/emulator-snapshots`

#### Genymotion Desktop

Сильные стороны: `gmtool`, QEMU/VirtualBox, quick boot, root toggle, настраиваемые CPU/RAM,
install/pull/ADB.

Причина отказа: нет официально подтверждённой встроенной ARM translation для Huawei Health.
Community ARM translation packages не подходят для устанавливаемого продукта.

Документация:

- `https://docs.genymotion.com/tools/desktop/gmtool/`

#### WSA, Android-x86/QEMU, redroid/WSL

- Windows Subsystem for Android больше не является надёжно распространяемым продуктовым runtime.
- Android-x86/QEMU снова упирается в ARM-only Huawei Health.
- redroid/WSL потребует собственного WSL kernel/binder stack и создаст существенно более сложную
  установку, чем решаемая задача.

## Обязательный benchmark MuMu против LDPlayer

Оба кандидата устанавливаются по очереди в изолированные каталоги. Проигравший полностью
удаляется после выбора. Тест выполняется на одном и том же официальном Huawei Health APK.

### Измерения

| Проверка         | Метод                                                      | Проходной критерий                         |
| ---------------- | ---------------------------------------------------------- | ------------------------------------------ |
| Размер установки | размер install + VM directories                            | минимальный из рабочих кандидатов          |
| Cold boot        | старт → `sys.boot_completed=1` и готовый ADB               | измерить, без заранее заданного числа      |
| Warm boot        | snapshot/quick boot → готовый ADB                          | минимальный из рабочих кандидатов          |
| Idle RAM/CPU     | Windows process tree после стабилизации                    | минимальный без потери sync                |
| Hidden execution | скрыть HWND, открыть день через ADB, дождаться file change | cache продолжает загружаться               |
| Private data     | root shell + listing/pull Huawei directory                 | полный read access                         |
| Login            | root off, ручной Huawei ID login                           | login и cloud data работают                |
| Root after login | включить root после сохранённой сессии                     | сессия не инвалидируется                   |
| UI automation    | `uiautomator dump`/accessibility nodes                     | календарь и дни имеют стабильные selectors |
| Recovery         | kill VM в середине дня, запустить снова                    | продолжение с checkpoint                   |

### Порядок проверки root

Huawei Health может отказываться работать на rooted device. Поэтому проверяются варианты:

1. root выключен во время login и обычного запуска;
2. после сохранения сессии root включается только для extraction;
3. если переключение требует reboot — проверить сохранность сессии;
4. если Huawei обнаруживает root и блокируется — попробовать offline extraction после завершения
   Huawei процесса;
5. если ни один вариант не работает, кандидат отбрасывается независимо от его скорости.

## Поиск фактического источника данных

Нельзя заранее зашивать путь к конкретной SQLite DB. Первый proof должен определить источник по
изменениям.

### File-diff protocol

1. Выбрать в аккаунте день с заранее известными данными.
2. Завершить Huawei Health и дождаться flush.
3. Снять manifest для `/data/user/0/com.huawei.health`:
   path, size, mtime и hash небольших файлов.
4. Запустить Huawei Health и открыть выбранный день.
5. Дождаться окончания loading и снова завершить приложение.
6. Снять второй manifest и diff.
7. Отдельно сохранить изменившиеся DB, `-wal`, `-shm`, JSON/protobuf и preferences.
8. Повторить с другим днём, чтобы отделить health data от telemetry/cache шума.

### Proof результата

До автоматизации календаря необходимо доказать:

- найден файл или набор файлов, содержащий данные выбранного дня;
- формат можно стабильно прочитать;
- timestamp/timezone и source ID понятны;
- повторное открытие дня не создаёт другую логическую запись;
- один и тот же extracted набор дважды импортируется в ARK без дублей.

Если данные зашифрованы device-bound ключом и недоступны даже с root, следующий объект
исследования — уже расшифрованный cache/runtime response после открытия дня, но не перехват
Huawei ID credentials.

## UI automation

Порядок предпочтения:

1. UIAutomator/accessibility resource-id и content-description;
2. текстовые selectors с зафиксированной локалью эмулятора;
3. иерархия view + относительная позиция внутри календаря;
4. screenshot/template matching только для элементов без accessibility tree;
5. координаты — последний fallback для фиксированных resolution/DPI и версии Huawei Health.

Во время обычной синхронизации окно VM скрыто. Bridge управляет Huawei Health через ADB, а не
эмулирует мышь Windows. Окно показывается только при необходимости ручного login/2FA, принятия
обновлённого соглашения или восстановления после изменившегося UI.

## Синхронизация

### Первый импорт

- Получить доступный диапазон дат из UI.
- Обходить даты/месяцы в обратном порядке.
- Если календарь показывает наличие данных, открывать только отмеченные дни.
- Если наличие данных до открытия определить нельзя, обходить каждый день.
- После каждой успешно разобранной даты записывать checkpoint.
- Ограничивать скорость, ждать фактическое окончание network loading и поддерживать resume после
  остановки Kosmos/VM.

### Последующие импорты

- Хранить `lastSuccessfulSourceTime` и последний завершённый календарный день.
- Начинать с небольшого overlap до последней синхронизации, чтобы подхватывать позднюю загрузку и
  исправления Huawei.
- Upsert делать по стабильному provider/source ID, а при его отсутствии — по детерминированному
  составному ключу.
- Не удалять ARK-данные только потому, что Huawei временно не отдал день.
- Применять delete только при найденном явном source tombstone/признаке удаления.

### ARK contract

- Renderer не читает файлы эмулятора и не пишет SQLite.
- Huawei Bridge работает в `platform/runtime` либо как управляемый runtime child.
- Нормализованные записи пишутся через `ark_core::db` с корректным sync metadata.
- Source-owned поля можно обновлять повторной синхронизацией.
- Пользовательские поля/заметки поверх импортированной записи не должны стираться.
- Сырые credentials Huawei не попадают в Kosmos, логи или ARK.

## Установка из Kosmos

Предполагаемый UX интеграции:

1. «Установить Huawei Bridge».
2. Проверка аппаратной виртуализации и свободного места.
3. Загрузка официального installer выбранного эмулятора.
4. Проверка Windows Authenticode publisher и ожидаемого download origin.
5. Запуск installer; полностью silent режим используется только если он официально поддерживается.
6. Создание отдельной VM и применение минимальной конфигурации.
7. Загрузка официального Huawei Health APK с Huawei.
8. Показ VM для ручного login.
9. Проверка доступа к известному дню.
10. Сохранение snapshot и переход к скрытым синхронизациям.

Kosmos не должен перепаковывать или хранить Huawei Health APK и proprietary emulator installer в
репозитории. Загружаются официальные актуальные artifacts. При удалении Bridge пользователь отдельно
выбирает: удалить VM/runtime или только отключить интеграцию. Импортированные ARK-данные по
умолчанию сохраняются.

## Минимизация runtime

После выбора победителя измеряется, а не предполагается, влияние каждого отключения:

- 2 vCPU;
- 1536 или 2048 МБ RAM — минимальное стабильное значение;
- фиксированное небольшое resolution/DPI;
- audio/camera/microphone/GPS/notifications выключены, если login/sync от них не зависят;
- отключены auto-start gaming services, store recommendations и launcher telemetry, насколько это
  допускает установленный runtime;
- ненужные Android packages сначала `pm disable-user --user 0`, физическое удаление только после
  доказательства, что Huawei login/sync не сломаны;
- после очистки выполняется compact виртуального диска, если формат VM это поддерживает;
- хранится один рабочий snapshot, а не цепочка snapshots.

Нельзя считать удаление system APK экономией места, пока физический virtual disk действительно не
уменьшился. Основная оптимизация — не запускать VM постоянно: быстрый warm start → sync → clean
shutdown.

## Безопасность и приватность

- ADB слушает только loopback.
- Huawei login выполняется пользователем непосредственно внутри Huawei Health.
- Kosmos не автоматизирует ввод пароля, CAPTCHA или 2FA.
- VM/data directory получает ACL текущего Windows user.
- Health data не отправляются во внешние сервисы Kosmos.
- В логах запрещены tokens, cookies, Huawei ID, полные health payloads и содержимое private DB.
- Debug bundle содержит только версии, timings, paths без username и redacted ошибки.
- Любой автоматический emulator update должен иметь rollback/snapshot, иначе можно потерять
  авторизованную сессию.

## Основные риски

1. Huawei Health обнаруживает root/emulator и блокирует login или cloud data.
2. MuMu/LDPlayer не продолжают UI/network работу при скрытом HWND.
3. Huawei использует encrypted/device-bound DB, а открытие дня оставляет только краткоживущий
   расшифрованный state.
4. Calendar UI не доступен через accessibility и меняется между версиями.
5. Huawei принудительно обновляет приложение и ломает parser/selectors.
6. Proprietary emulator installer не имеет пригодного silent/config contract.
7. Полный первый импорт требует открыть тысячи дней и занимает много времени.
8. Очистка gaming packages ломает HMS/Huawei ID dependency.

Каждый риск проверяется в spike до реализации полноценного UI Kosmos.

## Критерий выбора эмулятора

Кандидат допускается в реализацию только если одновременно:

1. Huawei login и загрузка известного дня проходят.
2. Private app-data читается воспроизводимо.
3. VM продолжает синхронизацию со скрытым окном.
4. UI automation может открыть две заданные даты без координатного happy-path.
5. Warm start и resource usage приемлемы для запуска по расписанию.
6. Установка и удаление управляемы из Kosmos без ручной настройки Android Studio.

Если проходят оба кандидата, выбирается меньший по disk footprint, затем по warm-start time, затем
по idle RAM. Дополнительные функции игрового эмулятора преимуществом не считаются.

## Следующий шаг при возврате к задаче

1. Установить MuMu Player 12 и LDPlayer 9 по очереди в изолированные каталоги.
2. Заполнить benchmark-таблицу фактическими числами.
3. Удалить проигравший runtime.
4. На победителе выполнить Huawei login и file-diff одного известного дня.
5. Не начинать Kosmos UI/installer до успешного one-day extraction proof.
