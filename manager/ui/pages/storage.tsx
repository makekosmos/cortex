import { Text, View, signal } from "nsh";
import { Card, CardRow, PillButton, SectionLabel } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { StoragePart } from "../state";
import { Page } from "../page";

/**
 * The storage overview of the old Manager: one bar, one row per part, and a
 * press that opens a part to show what is inside it. The bar is the page's
 * index — the same colour marks a segment and its row.
 */

/** One colour per part, as the old Manager had them. Stable by id, so a part
 * keeps its colour when the sizes move. */
function partColor(id: string): string {
  switch (id) {
    case "database":
    case "objects":
      return "#f09c0e";
    case "backups":
    case "sqlite_reserved":
    case "free_pages":
      return "#d9b852";
    case "dictation_models":
    case "types":
      return "#9b87db";
    case "packages":
    case "sync":
      return "#659bc8";
    case "updates":
      return "#80ab73";
    case "file_index":
    case "search_index":
      return "#65b9b0";
    case "app_index":
    case "migrations":
      return "#b79a7e";
    case "installations":
    case "links":
      return "#ce8293";
    case "cache":
    case "local_state":
      return "#88acbe";
    case "logs":
      return "#a7a7a7";
    case "settings":
    case "integrations":
      return "#ab9cb8";
    case "runtime_state":
    case "schema":
      return "#8f9b7a";
    case "recordings":
    case "usage_tracker":
      return "#d79067";
    default:
      return id.startsWith("database_file") ? "#88acbe" : "#89919b";
  }
}

/** When a backup was taken, as a list row reads it. */
function stamp(milliseconds: number): string {
  if (!milliseconds) return "";
  const date = new Date(milliseconds);
  const day = String(date.getDate()).padStart(2, "0");
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const rest = String(date.getMinutes()).padStart(2, "0");
  return `${day}.${month}.${date.getFullYear()} · ${hours}:${rest}`;
}

/** Bytes as a row reads them: a decimal unit and one significance step. */
function sizeText(bytes: number): string {
  const units = ["Б", "КБ", "МБ", "ГБ", "ТБ"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return `${bytes} Б`;
  return value >= 10 ? `${Math.round(value)} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

/** Biggest first, so the bar leads with what fills the disk. */
function sorted(parts: StoragePart[]): StoragePart[] {
  return [...parts].sort((left, right) => right.bytes - left.bytes);
}

/** The bar's segments. In the database scope the bar shows the database's own
 * files instead of the database as one part. */
function segments(parts: StoragePart[], databaseOnly: boolean): StoragePart[] {
  if (!databaseOnly) return sorted(parts);
  const database = parts.find((part) => part.id === "database");
  if (!database) return [];
  const items = (database.detail ?? []).map((item, index) => ({
    ...item,
    id: item.id || `database_file_${index}`,
  }));
  if (items.length === 0) return [database];
  const attributed = items.reduce((sum, item) => sum + item.bytes, 0);
  const remainder = database.bytes - attributed;
  if (remainder > 0) {
    items.push({
      id: "sqlite_reserved",
      label: "Журнал и резерв SQLite",
      description: "Журнал изменений, память координации и свободные страницы SQLite.",
      bytes: remainder,
      size: sizeText(remainder),
    });
  }
  return sorted(items);
}

const databaseOnly = signal(false);
const openIds = signal<string[]>([]);

function toggle(id: string) {
  const open = openIds();
  openIds.set(open.includes(id) ? open.filter((entry) => entry !== id) : [...open, id]);
}

export function StoragePage(props: PageProps) {
  const state = () => props.state();
  const parts = () => segments(state().storage.parts, databaseOnly());
  const total = () => parts().reduce((sum, part) => sum + part.bytes, 0);
  const open = (id: string) => openIds().includes(id);
  return (
    <Page colors={props.colors} title="Хранилище">
      <View class="flex-row" style={{ gap: 8 }}>
        <PillButton
          colors={props.colors}
          tone={databaseOnly() ? undefined : "accent"}
          label="Все"
          onPress={() => databaseOnly.set(false)}
        />
        <PillButton
          colors={props.colors}
          tone={databaseOnly() ? "accent" : undefined}
          label="База данных"
          onPress={() => databaseOnly.set(true)}
        />
      </View>

      <Card colors={props.colors} padded>
        <View style={{ gap: 4, marginBottom: 12 }}>
          <Text
            style={{
              fontSize: 28,
              lineHeight: 34,
              fontWeight: 600,
              color: props.colors.foreground,
            }}
          >
            {sizeText(total())}
          </Text>
          <Text style={{ fontSize: 12, lineHeight: 16, color: props.colors.muted }}>
            {databaseOnly()
              ? "Содержимое рабочей базы ARK и журнал SQLite"
              : "Установки, данные и локальные ИИ-модели Mundus"}
          </Text>
        </View>

        <View class="flex-row" style={{ height: 27, gap: 2, marginBottom: 14 }}>
          {parts().map((part) => (
            <View
              key={part.id}
              style={{
                flexGrow: total() > 0 ? (part.bytes / total()) * 100 : 1,
                minWidth: 4,
                height: 27,
                borderRadius: 5,
                backgroundColor: partColor(part.id),
              }}
              hoverStyle={{ opacity: 0.72 }}
              tooltip={
                <Text style={{ fontSize: 12, color: "#ffffff" }}>
                  {`${part.label}: ${sizeText(part.bytes)}`}
                </Text>
              }
              onPress={() => {
                const id = part.id;
                if (!open(id) && part.detail?.length) toggle(id);
              }}
            />
          ))}
        </View>

        <View>
          {parts().map((part) => (
            <View key={part.id}>
              <View
                class="flex-row items-center"
                style={{
                  gap: 10,
                  paddingVertical: 9,
                  borderRadius: 8,
                  paddingHorizontal: 6,
                }}
                hoverStyle={{ backgroundColor: "rgba(255,255,255,0.05)" }}
                onPress={() => {
                  if (part.detail?.length) toggle(part.id);
                }}
              >
                <View
                  style={{
                    width: 16,
                    height: 16,
                    borderRadius: 8,
                    backgroundColor: partColor(part.id),
                  }}
                />
                <View style={{ flexGrow: 1, flexShrink: 1, minWidth: 0 }}>
                  <Text
                    style={{
                      fontSize: 13,
                      lineHeight: 18,
                      color: props.colors.foreground,
                    }}
                  >
                    {part.label}
                  </Text>
                </View>
                <Text class="font-mono" style={{ fontSize: 12.5, color: props.colors.muted }}>
                  {sizeText(part.bytes)}
                </Text>
              </View>
              {open(part.id) && part.detail?.length ? (
                <View style={{ paddingLeft: 32, paddingBottom: 8 }}>
                  {part.detail.map((item, index) => (
                    <View
                      key={`${part.id}-${item.id}-${index}`}
                      class="flex-row items-center"
                      style={{ gap: 10, paddingVertical: 4 }}
                    >
                      <View style={{ flexGrow: 1, flexShrink: 1, minWidth: 0 }}>
                        <Text
                          style={{
                            fontSize: 12,
                            lineHeight: 16,
                            color: props.colors.muted,
                          }}
                        >
                          {item.label || item.id}
                        </Text>
                      </View>
                      <Text class="font-mono" style={{ fontSize: 12, color: props.colors.muted }}>
                        {sizeText(item.bytes)}
                      </Text>
                    </View>
                  ))}
                </View>
              ) : null}
            </View>
          ))}
        </View>
      </Card>

      <View>
        <SectionLabel colors={props.colors}>Данные</SectionLabel>
        <Card colors={props.colors}>
          <View style={{ paddingVertical: 10, gap: 2 }}>
            <Text
              style={{
                fontSize: 13,
                lineHeight: 18,
                color: props.colors.foreground,
              }}
            >
              Папка данных
            </Text>
            <Text
              style={{
                fontSize: 11.5,
                lineHeight: 16,
                color: props.colors.muted,
              }}
            >
              {state().storage.dataDir}
            </Text>
          </View>
        </Card>
      </View>

      <View>
        <SectionLabel colors={props.colors}>Резервные копии</SectionLabel>
        <Card colors={props.colors}>
          {state().backups.length ? (
            state().backups.map((backup, index, list) => (
              <CardRow
                key={backup.id}
                colors={props.colors}
                title={backup.id}
                subtitle={[stamp(backup.modified), backup.size].filter(Boolean).join(" · ")}
                last={index === list.length - 1}
                right={
                  <PillButton
                    colors={props.colors}
                    tone="danger"
                    label="Восстановить"
                    onPress={() => props.onCommand("backups.restore", { id: backup.id })}
                  />
                }
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              title="Копий нет"
              subtitle="Engine ещё не сделал ни одной"
              last
            />
          )}
          <CardRow
            colors={props.colors}
            title="Сделать копию сейчас"
            subtitle="Снимок базы рядом с данными Engine"
            last
            right={
              <PillButton
                colors={props.colors}
                tone="accent"
                label="Создать"
                onPress={() => props.onCommand("backups.create")}
              />
            }
          />
        </Card>
      </View>
    </Page>
  );
}
