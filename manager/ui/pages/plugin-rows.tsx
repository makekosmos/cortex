import { View } from "nsh";
import { CardRow, PillButton } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import type { PluginRow } from "../state";

function versionLine(plugin: PluginRow): string {
  const parts: string[] = [];
  if (plugin.version) parts.push(`Версия ${plugin.version}`);
  if (plugin.running) parts.push("запущен");
  return parts.join(" · ");
}

/** A row of the installed list: change its settings, remove it. A plugin is a
 * setting of this program, not a program beside it, so nothing here launches
 * one. */
export function InstalledRow(props: {
  plugin: PluginRow;
  colors: ApplicationTheme;
  last: boolean;
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
          <PillButton colors={colors} tone="danger" label="Удалить" onPress={props.onRemove} />
        </View>
      }
    />
  );
}

/** One catalog entry the Engine has not installed yet. */
export function AvailableRow(props: {
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
