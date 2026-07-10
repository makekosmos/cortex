// Чистая функция фильтрации объектов для live-обновления Eden-списка.
//
// Изолирована от Vue и внешних зависимостей — легко тестируется через bun test.
// Логика полностью повторяет listEntries() в kepler-api-shim.ts:
// — shouldIncludeObjectInEdenList + readVisibleObjectTypeIds-фильтр.

const COLLECTION_TYPE_ID = "collection_obj";

// Типы, для которых collection_obj НЕ показывается в Eden-списке.
// Синхронизировать с HIDDEN_EDEN_COLLECTION_TYPE_IDS в systemTypes.ts.
const HIDDEN_COLLECTION_TARGET_IDS = new Set([
  COLLECTION_TYPE_ID,
  "blocklist_obj",
  "tag_obj",
  "task_obj",
  "time_entry_obj",
]);

/**
 * Проверяет, должен ли объект включаться в Eden-список при live-событии.
 * Логика идентична listEntries():
 *
 * При пустом visibleTypeIds (дефолт): загружаются все типы через
 * list_object_summaries; не-collection типы проходят shouldIncludeObjectInEdenList=true.
 * Коллекции — по фильтру HIDDEN_COLLECTION_TARGET_IDS.
 *
 * При непустом visibleTypeIds: принимается только тип из этого списка
 * (или collection_obj с видимым и не-скрытым object_type_id).
 */
export function shouldIncludeTypeInEdenListForLiveUpdate(opts: {
  typeId: string;
  propsJson: Record<string, unknown>;
  visibleTypeIds: string[];
}): boolean {
  const { typeId, propsJson, visibleTypeIds } = opts;

  if (typeId === "system-type-journal" && propsJson.entry_kind === "bubble") return false;

  if (typeId === COLLECTION_TYPE_ID) {
    // Коллекции всегда проходят дополнительный фильтр по object_type_id.
    const objectTypeId = propsJson.object_type_id;
    if (typeof objectTypeId !== "string" || !objectTypeId) return false;
    if (HIDDEN_COLLECTION_TARGET_IDS.has(objectTypeId)) return false;
    // При непустом visibleTypeIds — object_type_id должен быть в списке.
    if (visibleTypeIds.length > 0 && !visibleTypeIds.includes(objectTypeId)) return false;
    return true;
  }

  if (visibleTypeIds.length === 0) {
    // Дефолт: принимаем все не-collection типы (аналог shouldIncludeObjectInEdenList=true).
    return true;
  }

  return visibleTypeIds.includes(typeId);
}
