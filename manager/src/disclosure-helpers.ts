import type { PackageDisclosure, StoreListing } from "./manager-api";

export type DisclosureSection = {
  title: string;
  lines: string[];
};

const CAPABILITY_LABELS = new Map([
  ["ark.read", "Читает данные Kosmos"],
  ["ark.write", "Создаёт и изменяет данные Kosmos"],
  ["launcher.search", "Ищет в лаунчере"],
  ["network", "Выходит в интернет"],
  ["filesystem.read", "Читает файлы"],
  ["filesystem.write", "Изменяет файлы"],
  ["clipboard", "Использует буфер обмена"],
  ["notifications", "Показывает уведомления"],
  ["process.spawn", "Запускает программы"],
  ["worker.invoke", "Вызывает операции других пакетов"],
  ["dictation.control", "Управляет диктовкой"],
]);

const ACTION_VERBS = new Map([
  ["read", "читает"],
  ["create", "создаёт"],
  ["update", "изменяет"],
  ["delete", "удаляет"],
  ["subscribe", "отслеживает изменения"],
  ["link", "связывает"],
]);

const DIRECTION_LABELS = new Map([
  ["import", "отдаёт данные в Kosmos"],
  ["export", "получает данные из Kosmos"],
  ["bidirectional-sync", "синхронизирует данные в обе стороны"],
]);

const FIDELITY_LABELS = new Map([
  ["native", "нативный формат"],
  ["lossless", "без потерь"],
  ["lossy", "частично"],
  ["metadata-only", "только метаданные"],
]);

export function capabilityLabel(capability: string) {
  return CAPABILITY_LABELS.get(capability) ?? capability;
}

export function typeLabel(typeId: string) {
  const last = typeId.split(".").at(-1) ?? typeId;
  const trimmed = last.replace(/_obj$/, "").replaceAll("_", " ");
  return trimmed || typeId;
}

function scopeSuffix(scopes: string[]) {
  return scopes.length ? `: ${scopes.join(", ")}` : "";
}

export function disclosureSections(disclosure: PackageDisclosure): DisclosureSection[] {
  const data = disclosure.data.map((rule) => {
    const verbs = rule.actions.map((action) => ACTION_VERBS.get(action) ?? action);
    const fields = [
      ...rule.fields_read.map((field) => `читает ${field}`),
      ...rule.fields_write.map((field) => `изменяет ${field}`),
      ...rule.relations_read.map((relation) => `читает связь ${relation}`),
      ...rule.relations_write.map((relation) => `связывает ${relation}`),
    ];
    const detail = fields.length ? ` (${fields.join(", ")})` : "";
    return `${typeLabel(rule.type)} — ${verbs.join(", ") || "без действий"}${detail}`;
  });
  const capabilities = disclosure.capabilities.map(
    (permission) => `${capabilityLabel(permission.capability)}${scopeSuffix(permission.scopes)}`,
  );
  const mappings = disclosure.mappings.map((mapping) => {
    const direction = DIRECTION_LABELS.get(mapping.direction) ?? mapping.direction;
    const fidelity = FIDELITY_LABELS.get(mapping.fidelity);
    return `${typeLabel(mapping.type)} — ${direction}${fidelity ? ` (${fidelity})` : ""}`;
  });
  return [
    { title: "Данные", lines: data },
    { title: "Доступы", lines: capabilities },
    { title: "Обмен данными", lines: mappings },
  ].filter((section) => section.lines.length > 0);
}

export function needsStoreDisclosure(listing: StoreListing, development = false) {
  return !development && listing.publisher_tier !== "kosmos";
}
