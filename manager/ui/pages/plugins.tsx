import { effect, signal, Text, View } from "nsh";
import {
  Card,
  CardRow,
  FieldRow,
  PillButton,
  SectionLabel,
  StatusDot,
  Switch,
  TextInput,
} from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { SettingValue } from "../state";
import { hotkeyCaptured } from "../state";
import {
  CHIPS,
  ChoiceRow,
  LABEL_WIDTH,
  PickerModal,
  PickerRow,
  picking,
  same,
} from "./plugin-fields";
import { Page } from "../page";
import { Modal, note } from "../modal";
import { HotkeyRecorder } from "./plugin-hotkey";
import { AvailableRow, InstalledRow } from "./plugin-rows";
import { StatsBlock } from "./plugin-stats";

/**
 * What Mundus can gain and what it already has.
 *
 * The catalog is the Engine's (`plugins.catalog`), the install is its too
 * (`plugins.install`), and this page only names what it is told — a new plugin
 * appears here because it is in the catalog, not because the Manager was built
 * knowing it.
 */
export function PluginsPage(props: PageProps) {
  const state = () => props.state().plugins;
  const installed = () => state().installed;
  const available = () => state().catalog.available;
  const isInstalled = (id: string) => installed().some((plugin) => plugin.id === id);
  const rest = () => available().filter((plugin) => !isInstalled(plugin.id));
  return (
    <Page colors={props.colors} title="Плагины">
      <View>
        <SectionLabel colors={props.colors}>Установленные</SectionLabel>
        <Card colors={props.colors}>
          {installed().length ? (
            installed().map((plugin, index) => (
              <InstalledRow
                key={plugin.id}
                plugin={plugin}
                colors={props.colors}
                last={index === installed().length - 1}
                onOpen={() => props.onCommand("plugins.launch", { id: plugin.id })}
                onConfigure={() => props.onCommand("route", { route: `plugin/${plugin.id}` })}
                onRemove={() => props.onCommand("plugins.remove", { id: plugin.id })}
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              icon="plug"
              iconFill={props.colors.raised}
              accent={props.colors.muted}
              title="Пока ничего не установлено"
              subtitle="Выберите плагин из списка ниже"
              last
            />
          )}
        </Card>
      </View>

      <View>
        <SectionLabel
          colors={props.colors}
          right={
            state().catalog.stale ? (
              <Text style={{ fontSize: 12, color: props.colors.muted }}>
                Список сохранён: Engine не смог обновить его
              </Text>
            ) : null
          }
        >
          Доступные
        </SectionLabel>
        <Card colors={props.colors}>
          {rest().length ? (
            rest().map((plugin, index) => (
              <AvailableRow
                key={plugin.id}
                plugin={plugin}
                colors={props.colors}
                last={index === rest().length - 1}
                onInstall={() => props.onCommand("plugins.install", { id: plugin.id })}
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              icon="check"
              iconFill={props.colors.raised}
              accent={props.colors.muted}
              title={available().length ? "Всё установлено" : "Список пуст"}
              subtitle={
                available().length
                  ? "Новые плагины появятся здесь сами"
                  : "Engine не получил каталог плагинов"
              }
              last
            />
          )}
        </Card>
        {rest().length ? (
          <View style={{ marginTop: 8, paddingLeft: 12 }}>
            <Text style={{ fontSize: 12, lineHeight: 16, color: props.colors.muted }}>
              Загрузка идёт в фоне — плагин появится в списке выше, когда Engine его установит.
            </Text>
          </View>
        ) : null}
      </View>
    </Page>
  );
}

/** The values a page has typed but not sent yet, per plugin. */
const drafts = signal<{ id: string; values: Record<string, SettingValue> }>({
  id: "",
  values: {},
});

/**
 * One plugin's page, drawn from its manifest: the Engine stores the settings,
 * the plugin reads them from the file the manifest names, and this page writes
 * nothing the manifest does not declare.
 */
export function PluginPage(props: PageProps & { id: string }) {
  const current = () => props.state().plugins.current;
  const manifest = () => {
    const plugin = current();
    return plugin && plugin.id === props.id ? plugin.manifest : null;
  };
  const stored = () => {
    const plugin = current();
    return plugin && plugin.id === props.id ? plugin.values : {};
  };
  const typed = () => (drafts().id === props.id ? drafts().values : {});
  const dirty = () => Object.keys(typed()).length > 0;
  const valueOf = (key: string): SettingValue | undefined =>
    key in typed() ? typed()[key] : stored()[key];
  const draft = (key: string, value: SettingValue) =>
    drafts.set({ id: props.id, values: { ...typed(), [key]: value } });
  const send = (key: string, value: SettingValue) =>
    props.onCommand("plugins.set", { id: props.id, values: { [key]: value } });
  const save = () => {
    if (!dirty()) return;
    props.onCommand("plugins.set", { id: props.id, values: typed() });
    drafts.set({ id: props.id, values: {} });
  };

  // The tap in the Manager reads the combination, not this window: it arrives
  // as an event and is stored the same way a typed setting is.
  effect(() => {
    const captured = hotkeyCaptured();
    if (!captured) return;
    hotkeyCaptured.set("");
    send("hotkey", captured);
  });

  const plugin = () => props.state().plugins.installed.find((row) => row.id === props.id);
  const stats = () => {
    const row = current();
    return row && row.id === props.id ? (row.stats ?? null) : null;
  };
  const shown = manifest();
  /** The choice whose list is open, if any: `picking` holds its key. */
  const pickingField = () => {
    const key = picking().key;
    const field = shown?.settings.find((row) => row.key === key);
    if (!field || field.kind.type !== "choice") return null;
    return { key: field.key, label: field.label, options: field.kind.options };
  };

  if (!shown) {
    // The answer takes a moment; until it lands the page says so, and a page
    // that stays empty offers a reload rather than a dead end.
    const waiting = current()?.id === props.id && current()?.loading;
    return (
      <Page colors={props.colors} title={plugin()?.name ?? "Плагин"}>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="plug"
            iconFill={props.colors.raised}
            accent={props.colors.muted}
            title={waiting ? "Загружаем настройки" : "Настройки недоступны"}
            subtitle={
              waiting
                ? "Ждём ответа Engine"
                : "Engine не сообщил описание плагина — нажмите, чтобы повторить"
            }
            onPress={waiting ? undefined : () => props.onCommand("route.reload", {})}
            last
          />
        </Card>
      </Page>
    );
  }

  return (
    <>
      <Page
        colors={props.colors}
        title={shown.name}
        actions={
          dirty() ? (
            <View style={{ marginLeft: 12 }}>
              <PillButton colors={props.colors} tone="accent" label="Сохранить" onPress={save} />
            </View>
          ) : null
        }
      >
        <View>
          <SectionLabel colors={props.colors}>Плагин</SectionLabel>
          <Card colors={props.colors}>
            <CardRow
              colors={props.colors}
              icon="plug"
              accent={props.colors.accent}
              accentTo={props.colors.accent}
              title={shown.name}
              subtitle={shown.summary}
              last
              right={
                <View class="flex-row items-center gap-2">
                  {plugin()?.running ? <StatusDot colors={props.colors} /> : null}
                  {plugin()?.executable ? (
                    <PillButton
                      colors={props.colors}
                      tone="neutral"
                      label="Открыть"
                      onPress={() => props.onCommand("plugins.launch", { id: props.id })}
                    />
                  ) : null}
                </View>
              }
            />
          </Card>
        </View>

        {stats() ? <StatsBlock stats={stats()!} colors={props.colors} /> : null}

        <View>
          <SectionLabel colors={props.colors}>Настройки</SectionLabel>
          <Card colors={props.colors}>
            {shown.settings.map((field, index) => {
              const last = index === shown.settings.length - 1;
              const value = valueOf(field.key);
              if (field.kind.type === "toggle") {
                return (
                  <CardRow
                    key={field.key}
                    colors={props.colors}
                    title={field.label}
                    subtitle={field.hint}
                    last={last}
                    right={
                      <Switch
                        colors={props.colors}
                        value={Boolean(value)}
                        accessibilityLabel={field.label}
                        onChange={(on) => send(field.key, on)}
                      />
                    }
                  />
                );
              }
              const kind = field.kind;
              if (kind.type === "switch") {
                // One switch, two stored values: the app's "insert automatically"
                // is `auto_paste` or `clipboard_only`, not `true`/`false`.
                return (
                  <CardRow
                    key={field.key}
                    colors={props.colors}
                    title={field.label}
                    subtitle={field.hint}
                    last={last}
                    right={
                      <Switch
                        colors={props.colors}
                        value={!same(kind.off, value) && Boolean(value)}
                        accessibilityLabel={field.label}
                        onChange={(on) => send(field.key, on ? kind.on : kind.off)}
                      />
                    }
                  />
                );
              }
              if (kind.type === "choice") {
                const options = kind.options;
                if (field.key === "localModel" || options.length > CHIPS) {
                  return (
                    <PickerRow
                      key={field.key}
                      fieldKey={field.key}
                      label={field.label}
                      options={options}
                      value={value}
                      colors={props.colors}
                      last={last}
                    />
                  );
                }
                return (
                  <ChoiceRow
                    key={field.key}
                    label={field.label}
                    options={options}
                    value={value}
                    colors={props.colors}
                    last={last}
                    onPick={(next) => send(field.key, next)}
                  />
                );
              }
              const numeric = field.kind.type === "number";
              if (field.key === "hotkey") {
                const hotkey = () => props.state().hotkey;
                return (
                  <FieldRow
                    key={field.key}
                    colors={props.colors}
                    label={field.label}
                    last={last}
                    labelWidth={LABEL_WIDTH}
                  >
                    <HotkeyRecorder
                      value={String(value ?? "")}
                      colors={props.colors}
                      capturing={hotkey().capturing}
                      held={hotkey().held}
                      listenAccess={hotkey().listenAccess}
                      onStart={() => props.onCommand("hotkey.capture")}
                      onOpenSettings={() => props.onCommand("hotkey.settings")}
                    />
                  </FieldRow>
                );
              }
              return (
                <FieldRow
                  key={field.key}
                  colors={props.colors}
                  label={field.label}
                  last={last}
                  labelWidth={LABEL_WIDTH}
                >
                  <TextInput
                    colors={props.colors}
                    value={String(value ?? "")}
                    placeholder={numeric ? "Число" : "Не задано"}
                    accessibilityLabel={field.label}
                    onChange={(text) => draft(field.key, numeric ? Number(text) : text)}
                    onSubmit={() => {
                      const next = valueOf(field.key);
                      if (next !== undefined) send(field.key, next);
                      drafts.set({ id: props.id, values: {} });
                    }}
                  />
                </FieldRow>
              );
            })}
          </Card>
          {shown.settings.length ? null : note(props.colors, "У этого плагина нет настроек.")}
        </View>
      </Page>
      {(() => {
        // The key is captured here: the pick closes the list first, and a
        // handler that read `picking()` afterwards would send it nowhere.
        const picked = pickingField();
        return picked ? (
          <PickerModal
            title={picked.label}
            options={picked.options}
            value={valueOf(picked.key)}
            colors={props.colors}
            onPick={(next) => send(picked.key, next)}
          />
        ) : null;
      })()}
    </>
  );
}
