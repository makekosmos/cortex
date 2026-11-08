import { signal, Text, View } from "nsh";
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
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { PluginRow, SettingValue } from "../state";
import { Page } from "../page";
import { note } from "../modal";

function versionLine(plugin: PluginRow): string {
  const parts: string[] = [];
  if (plugin.version) parts.push(`Версия ${plugin.version}`);
  if (plugin.running) parts.push("запущен");
  return parts.join(" · ");
}

/** A row of the installed list: open it, change its settings, remove it. */
function InstalledRow(props: {
  plugin: PluginRow;
  colors: ApplicationTheme;
  last: boolean;
  onOpen: () => void;
  onConfigure: () => void;
  onRemove: () => void;
}) {
  const { colors, plugin } = props;
  return (
    <CardRow
      colors={colors}
      icon="plug"
      accent={colors.accent}
      accentTo={colors.accent}
      title={plugin.name}
      subtitle={versionLine(plugin) || plugin.summary}
      last={props.last}
      right={
        <View class="flex-row items-center gap-2">
          <PillButton colors={colors} label="Настроить" onPress={props.onConfigure} />
          {plugin.executable ? (
            <PillButton colors={colors} tone="neutral" label="Открыть" onPress={props.onOpen} />
          ) : null}
          <PillButton colors={colors} tone="danger" label="Удалить" onPress={props.onRemove} />
        </View>
      }
    />
  );
}

/** One catalog entry the Engine has not installed yet. */
function AvailableRow(props: {
  plugin: PluginRow;
  colors: ApplicationTheme;
  last: boolean;
  onInstall: () => void;
}) {
  const { colors, plugin } = props;
  return (
    <CardRow
      colors={colors}
      icon="apps"
      accent={colors.muted}
      accentTo={colors.muted}
      title={plugin.name}
      subtitle={plugin.summary}
      last={props.last}
      right={
        <PillButton colors={colors} tone="accent" label="Установить" onPress={props.onInstall} />
      }
    />
  );
}

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

/** A choice as a row of buttons: one is on, the rest ask to be. */
function ChoiceRow(props: {
  label: string;
  options: { value: string; label: string }[];
  value: string;
  colors: ApplicationTheme;
  last: boolean;
  onPick: (value: string) => void;
}) {
  const { colors, value } = props;
  return (
    <FieldRow colors={colors} label={props.label} last={props.last} labelWidth={LABEL_WIDTH}>
      <View class="flex-row flex-wrap items-center" style={{ gap: 6 }}>
        {props.options.map((option) => (
          <PillButton
            key={option.value}
            colors={colors}
            tone={option.value === value ? "accent" : "neutral"}
            label={option.label}
            onPress={() => props.onPick(option.value)}
          />
        ))}
      </View>
    </FieldRow>
  );
}

/** The label column: wide enough for a two-word setting name, so a field never
 * wraps into the control's row (`Подсказка для расшифровки` is the longest). */
const LABEL_WIDTH = 210;

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

  const plugin = () => props.state().plugins.installed.find((row) => row.id === props.id);
  const shown = manifest();

  if (!shown) {
    return (
      <Page colors={props.colors} title={plugin()?.name ?? "Плагин"}>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="plug"
            iconFill={props.colors.raised}
            accent={props.colors.muted}
            title="Настройки недоступны"
            subtitle="Engine не сообщил описание плагина"
            last
          />
        </Card>
      </Page>
    );
  }

  return (
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
                <PillButton
                  colors={props.colors}
                  tone="neutral"
                  label="Открыть"
                  onPress={() => props.onCommand("plugins.launch", { id: props.id })}
                />
              </View>
            }
          />
        </Card>
      </View>

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
            if (field.kind.type === "choice") {
              return (
                <ChoiceRow
                  key={field.key}
                  label={field.label}
                  options={field.kind.options}
                  value={String(value ?? "")}
                  colors={props.colors}
                  last={last}
                  onPick={(next) => send(field.key, next)}
                />
              );
            }
            const numeric = field.kind.type === "number";
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
  );
}
