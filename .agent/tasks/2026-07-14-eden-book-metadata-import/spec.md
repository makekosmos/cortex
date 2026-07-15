# Eden: локальное заполнение книги по ссылке

## Классификация

`FULL_LOOP`: новый пользовательский workflow затрагивает системную схему `book_obj`, sandboxed browser/IPC boundary, typed header, существующий ARK write-path, tests и source-документацию.

## Цель

Позволить пользователю открыть уже созданную книгу, нажать «Заполнить по ссылке», вставить публичную HTTPS-ссылку на страницу книги и локально извлечь доступные метаданные без Zyte, Apify, Google Books, Open Library, облачной AI или другого extraction-сервиса. Перед записью Eden показывает preview; подтверждённые данные сохраняются обычным ARK write-path книги.

## В scope

- optional поля `isbn`, `page_count`, `language`, `publisher`, `published_date`, `source_url` в `book_obj`;
- локальная SSRF-защищённая загрузка обычного HTML с изолированным Electron browser fallback для страниц с JS challenge;
- generic extraction из JSON-LD, OpenGraph/meta и видимого текста страницы;
- нормализация и checksum-проверка ISBN-10/ISBN-13;
- кнопка «Заполнить по ссылке», Visuals modal, progress/error/preview и явное применение;
- выбор найденных полей в preview и замена только подтверждённых title/header values с сохранением Markdown body;
- deterministic fixtures для LiveLib-подобной и Schema.org Book страниц, component tests и headless Electron visual evidence;
- source docs и generated docs sync.

## Вне scope

- локальная LLM, скачивание модели или универсальная AI-классификация типов объектов;
- внешний metadata/extraction API, proxy service или собственный облачный backend;
- обход CAPTCHA/WAF, автоматизация логина или импорт приватных страниц;
- собственный глобальный ISBN-каталог и заполнение данных, отсутствующих на исходной странице;
- рейтинг книги и выбор шкалы рейтинга;
- массовый импорт, browser extension, release/version bump;
- новый ARK RPC, SQLite migration, изменение sync-протокола или прямой SQL write.

## Acceptance Criteria

**AC1. Схема книги.** Системный `book_obj` сохраняет существующие `author`/`cover_image` и добавляет optional `isbn` (text), `page_count` (number), `language` (text), `publisher` (text), `published_date` (text), `source_url` (url). Поля живут в `header_props_json`, доступны обычному typed-header renderer и сохраняются без SQLite migration, нового ARK RPC или direct SQL.

**AC2. Локальный extractor.** Desktop capability принимает только публичный HTTPS URL. Быстрый путь использует ограниченный HTML fetch с DNS public-address validation, pinned IP, повторной проверкой redirects, timeout и byte cap. Если страница отвечает 403/429/503 или успешным HTML с распознанным browser challenge, fallback загружает её локально в ephemeral sandboxed/context-isolated Electron browser без Node integration и privileged preload, запрещает новые окна/downloads/permissions и ограничивает время/navigation scope. У fallback session нет direct network route: HTTPS/WSS идут через ephemeral local CONNECT proxy, который проверяет все DNS-адреса и открывает upstream socket к выбранному public IP без повторного hostname resolution; HTTP и private targets запрещены, а implicit loopback bypass Chromium снят. Никакой URL или HTML не отправляется стороннему extraction/AI service. Результат имеет типизированный контракт и может содержать title, authors, cover image, ISBN, pages, language, publisher, published date и source URL.

**AC3. Generic extraction.** Extractor сначала использует Schema.org/JSON-LD и OpenGraph/meta, затем детерминированно дополняет поля из видимого текста. ISBN очищается, проверяется checksum и при возможности нормализуется в ISBN-13; невалидные ISBN и некорректные page counts не применяются. Deterministic fixture в формате LiveLib извлекает название, автора, обложку, ISBN, страницы, язык, издательство и год без domain-specific LiveLib adapter.

**AC4. Workflow и preview.** На странице `book_obj` видна кнопка «Заполнить по ссылке». Она открывает `@kosmos/visuals` Modal с `TextInput` и Visuals Buttons. После «Найти данные» пользователь видит progress, затем preview найденных полей и кнопки «Отмена»/«Применить». Все найденные поля выбраны по умолчанию; пользователь может снять галочку с любого поля, которое не нужно заменять. Невалидная ссылка, недоступная/заблокированная страница и отсутствие книжных данных дают понятную русскую ошибку без изменения книги.

**AC5. Безопасное применение.** `BookMetadataImportModal` эмитит типизированный patch только из выбранных полей; `TypedHeader` только передаёт его наверх; существующий `TiptapEditor` остаётся единственным владельцем autosave/write-path. Выбранные title/header fields заменяют существующие значения, отсутствующие и снятые с выбора поля остаются без изменений, Markdown body не меняется, а заменённый локальный файл обложки удаляется только после успешного сохранения новой ссылки. При save failure persisted ARK object остаётся прежним, а optimistic draft остаётся dirty для штатного retry.

**AC6. Offline-first и cache.** Создание, открытие и ручное редактирование книги не зависит от сети. Импорт является явным best-effort действием. Повторный импорт того же неизменившегося URL в одной сессии может использовать локальный memory cache; отсутствие сети не блокирует остальные действия Eden.

**AC7. Доказательство.** Unit tests покрывают schema, ISBN validation/normalization и generic fixtures. Browser component tests покрывают modal input, progress, preview, error, apply emit и сохранение существующих значений. Headless Electron visual baseline показывает initial и preview states. `test:unit`, `test:vue`, `ark:guard:writes`, desktop JS build, docs sync/check и `visual:eden` проходят; независимый verifier подтверждает AC по текущему worktree.

## Проверки

```powershell
rtk bun run --cwd products/eden test:unit
rtk bun run --cwd products/eden test:vue
rtk bun run ark:guard:writes
rtk bun run --cwd platform/desktop build:js
rtk bun run docs:sync
rtk bun run docs:check
rtk bun run visual:eden --update-snapshots
rtk bun run visual:eden
```
