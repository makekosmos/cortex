# Live-обновление списков при создании объекта на другом устройстве

Дата: 2026-06-17
Классификация: FULL_LOOP (sync→UI live updates, два приложения).
Ветка: feat/iroh-transport.

## Симптом

Новый объект (заметка), созданный на другом устройстве, синкается в БД, но НЕ
появляется в списке («недавние» / коллекции) без перезагрузки страницы.
Обновление существующих и открытой заметки уже работает; не хватает именно
ПОЯВЛЕНИЯ нового и ИСЧЕЗНОВЕНИЯ удалённого в списках.

## Обзор (Haiku-survey, подтверждено)

| Приложение       | Список                                                     | Реакция на новый удалённый объект                 | Gap |
| ---------------- | ---------------------------------------------------------- | ------------------------------------------------- | --- |
| eden             | sidebar «недавние» + TypeObjectsView коллекции (`entries`) | только `currentEntry` обновляется                 | ДА  |
| platform/desktop | Dashboard ObjectTable                                      | подписки НЕТ, грузит раз при mount + кэш          | ДА  |
| delphi           | todo list                                                  | add/remove работает (`subscribeArkObjectChanges`) | нет |
| arrancador       | games                                                      | full refresh на `entity_changed`                  | нет |

Чиним только **eden** и **dashboard**.

## События (доставка работает, чинить только renderer)

Backend эмитит `object_upserted` / `object_deleted` (payload: `id`, `type_id`)
при локальной И входящей peer-записи. Доходит до renderer через kepler bridge
(`subscribe("object_upserted", ...)`) и шимы. ВАЖНО: событие общее для ВСЕХ
типов объектов (eden notes, delphi `task_obj`, arrancador `game_obj`, и т.д.).

## Часть A — Eden список

Файлы: `products/eden/src/store/eden.ts` (список `entries`,
`startLiveRefreshSubscription` ~685), `products/eden/src/lib/kepler-api-shim.ts`
(`listEntries` ~371, `loadEntry` ~426, фильтры `readVisibleObjectTypeIds`,
`shouldIncludeObjectInEdenList`).

Сейчас `startLiveRefreshSubscription` рано выходит, если `payload.id !==
currentEntry.id` — поэтому новый объект в `entries` не попадает.

Требование:

- На `object_upserted` для id, которого НЕТ в `entries.value`: добавить его в
  список — НО только если объект относится к Eden-списку (тот же фильтр, что
  `listEntries`: видимые типы + `shouldIncludeObjectInEdenList`). Чужие типы
  (`task_obj`, `game_obj`, и пр.) добавлять НЕЛЬЗЯ.
- На `object_upserted` для id, который УЖЕ в `entries`: обновить его в списке
  (свежие title/updated_at) — чтобы порядок/превью были актуальны. (Не путать с
  пере-гидратацией открытой заметки — та уже сделана и должна сохраниться.)
- На `object_deleted`: удалить из `entries.value` (и, если это `currentEntry`,
  закрыть — уже реализовано).
- НЕ ломать существующую логику открытой заметки (currentEntry re-hydrate,
  dirty-guard, self-echo guard). Реструктурируй обработчик: сначала
  список-уровень (любой id), потом currentEntry-уровень.

Подход к фильтрации (выбери надёжный):

- предпочтительно: добавить в шим функцию вида `loadListableEntry(id)` /
  `isObjectListable(id|typeId)`, которая переиспользует существующие
  `readVisibleObjectTypeIds` + `shouldIncludeObjectInEdenList`, и вернуть Entry
  только если объект подходит для списка (иначе undefined). Store вызывает её.
- допустимая (более простая) альтернатива: debounced полный `refreshData()` на
  событие для неизвестного/удалённого id (как arrancador). Минус — полный
  reload списка; если выберешь — сделай debounce (напр. 150–300мс), чтобы burst
  создаваемых объектов не дёргал refresh многократно.

Не дублируй подписку — расширь существующую `subscribeObjectChanges`-подписку.

## Часть B — Dashboard (platform/desktop)

Файлы: `platform/desktop/src/dashboard/store.ts` (кэш `objectRowsCache`,
`objects`), `platform/desktop/src/views/Dashboard*.vue` (загрузка при mount
~40-43). Подписки на live-события нет.

Требование:

- Подписаться на `object_upserted` / `object_deleted` (через тот же bridge/механизм,
  что используют другие desktop-вьюхи; посмотри, как доставляются ark events в
  renderer shell — `platform/desktop/electron` preload / kepler bridge).
- При событии: инвалидировать `objectRowsCache` для затронутого типа и обновить
  `objects.value`, если сейчас открыт этот тип (или просто перезагрузить текущий
  выбранный тип). Учитывай `type_id` из payload, чтобы не дёргать лишнее.
- Cleanup подписки при unmount (правило: нет подписок без cleanup).

## TDD / проверки

- Где логика чистая (например «подходит ли объект в Eden-список по type_id» или
  «нужно ли обновлять dashboard для этого type_id») — вынести и покрыть unit-
  тестом RED→GREEN.
- Eden: `cd products/eden && bun run test:unit` (pre-existing провалы
  `preferences.test.ts`/`vimMotions.test.ts` — подтвердить через git stash, что
  падали и до правок).
- Desktop: соответствующий typecheck/тест (посмотри package.json platform/desktop).
- Полная проверка UI — на 2 машинах пользователем (СТОП-точка): новый объект
  появляется в списке/таблице без reload; удалённый исчезает; чужие типы не
  протекают в Eden-список. НЕ объявлять UI PASS без visual verify.

## Ограничения

- НЕ коммить/пушить (родитель закоммитит по приложениям отдельно).
- НЕ попутный рефакторинг; минимальные изменения. Не добавлять лишние конфиги
  (никаких новых tsconfig). Не бампать версии. UI-строки на русском, цвета/шрифты
  — токены @kosmos/visuals если трогаешь визуал.
- Только renderer. НЕ трогать ARK/sidecar/protocol/схему.
