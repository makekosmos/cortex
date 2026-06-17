# Eden: открытие заметки ложно помечает её изменённой (bump updated_at)

Дата: 2026-06-17
Классификация: FULL_LOOP (Eden editor internals + взаимодействие с autosave/sync).
Ветка: feat/iroh-transport.

## Симптом

Первое открытие заметки помечает её как изменённую, хотя пользователь ничего не
правил: `updated_at` бампится, и заметка всплывает наверх ленты (список
сортируется по `updated_at desc`). Это и визуальный мусор, и риск: лишний
autosave может конфликтовать с реальными изменениями/входящим sync.

## Почему важно сейчас (связь с live-refresh)

В этой же сессии добавлен live-refresh открытой заметки с **dirty-guard**:
`isCurrentEntryDirty` (store/eden.ts) блокирует применение удалённых изменений,
пока редактор «грязный», чтобы не затереть ввод. Если открытие ЛОЖНО ставит
`dirty=true`, dirty-guard будет зря пропускать удалённые обновления для
просто-открытой заметки. Поэтому корректное «открытие ≠ dirty» обязательно.

## Диагноз (где копать)

- Список сортируется по `updated_at desc` — `store/eden.ts:87`.
- `updateEntryDraft` (`store/eden.ts:869`) ставит `isCurrentEntryDirty=true` и
  заменяет `entries[idx]` ТОЛЬКО если `hasUserVisibleEntryChanges` вернул true
  (иначе ранний `return` на ~877). Значит при открытии эмитится черновик
  (`entryDraftChange`), который эта функция считает реальным изменением.
- `hasUserVisibleEntryChanges` (`store/eden.ts:104`) сравнивает: `title`,
  `type_id`, `header_layout`, `header_props_json` (нормализованный), и markdown
  (`readEntryMarkdown`).
- CmEditor эмитит `entryDraftChange` (через `buildEntryDraft`, который ставит
  `updated_at: nextDraftUpdatedAt()`) в нескольких местах:
  `editor-cm/CmEditor.vue` ~319, ~343-344, ~359, ~380, ~448, ~549. Надо найти,
  какой из них срабатывает на ОТКРЫТИИ обычной заметки без пользовательского
  ввода. Подозреваемые на mount/open: `backfillPersonNameFromTitle` (onMounted
  ~564), инициализация/ресериализация `header_props` (`safeParseHeaderProps` →
  отличается от хранимого, напр. добавляются дефолтные ключи), нормализация
  `title` (`getEditableEntryTitle`), либо `updateListener` тела, срабатывающий
  на программную начальную установку (хотя есть `suppressBodySyncSave`).

## Цель / требуемое поведение

Открытие заметки (mount + первичная гидратация, без реального пользовательского
изменения title/header/тела) НЕ должно:

- эмитить `entryDraftChange`, который двигает `updated_at`/переставляет ленту;
- ставить `isCurrentEntryDirty=true`.

Реальные пользовательские правки (ввод текста, смена title/типа/header) —
по-прежнему помечают dirty и сохраняются как раньше. Ничего не регрессит.

## Указания (ориентир, решение — на усмотрение исполнителя)

Найди точный триггер (добавь временный лог в каждый emit-сайт или проследи по
коду) и устрани именно его. Предпочтительные подходы:

- Не эмитить draft на программной/инициализирующей гидратации (расширить
  guard по аналогии с `suppressBodySyncSave` на title/header backfill);
- ИЛИ сделать `hasUserVisibleEntryChanges` устойчивой к чисто-нормализационным
  отличиям (если разница только из-за ре序иализации дефолтов header_props/title,
  это НЕ user-visible change). Но осторожно: не замаскировать реальные правки.

Не «лечи симптом» бампом-в-обе-стороны: цель — чтобы открытие вообще не
порождало черновик.

## ОБЯЗАТЕЛЬНО прочитать

- `docs-site/apps/eden/editor.md`, `docs-site/apps/eden/data.md`,
  `docs-site/apps/eden/typed-notes.md`
- При необходимости `docs-site/apps/eden/postmortems.md` (там уже есть разборы
  autosave/draft гонок — §2026-06-15, §2026-06-16).

## TDD

Где логика чистая — покрой тестом (RED→GREEN). Например, если правишь
`hasUserVisibleEntryChanges`: тест, что entry, отличающийся только
нормализацией header_props/title-дефолтами, НЕ считается изменённым, а реальная
смена title/тела — считается. Прогони, увидь RED, затем GREEN.

## Acceptance

- Открытие заметки не двигает её в ленте и не ставит dirty (проверяемо логикой/
  тестом + визуально пользователем).
- Реальные правки сохраняются и сортируются как раньше (нет регресса autosave).
- Existing Eden unit-тесты зелёные (кроме заведомо pre-existing провалов
  `preferences.test.ts`/`vimMotions.test.ts` — подтвердить, что они падали и до
  правок).
- НЕ добавлять `products/eden/tsconfig.json` и прочие попутные файлы. Один
  логический change. Не бампать версии. UI-строки — на русском.
