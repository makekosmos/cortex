import { Text } from "nsh";
import { PillButton } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { ManagerState } from "../state";

/** The update card's one button: check, download, or install. Nothing is
 * downloaded without this press.
 *
 * A plain function, not a component: the host compares a child component's props
 * by identity, and the Engine patches one state object in place — a component
 * boundary here would keep the first button on screen after the state moved. */
export function updateAction(props: PageProps & { updates: ManagerState["updates"] }) {
  const { updates } = props;
  if (updates.percent > 0 && updates.percent < 100) {
    return (
      <Text
        style={{ fontSize: 12.5, color: props.colors.muted }}
      >{`${Math.round(updates.percent)}%`}</Text>
    );
  }
  // Ready to install with or without a version label: the Engine has the
  // package, and the subtitle says so.
  if (updates.canInstall) {
    return (
      <PillButton
        colors={props.colors}
        tone="accent"
        label="Установить"
        onPress={() => props.onCommand("updates.install")}
      />
    );
  }
  if (updates.newVersion) {
    return (
      <PillButton
        colors={props.colors}
        tone="accent"
        label="Скачать"
        onPress={() => props.onCommand("updates.download")}
      />
    );
  }
  return (
    <PillButton
      colors={props.colors}
      label="Проверить"
      onPress={() => props.onCommand("updates.check")}
    />
  );
}

export function updateSubtitle(updates: ManagerState["updates"]): string {
  if (updates.percent > 0 && updates.percent < 100)
    return `Загружается · ${Math.round(updates.percent)}%`;
  if (updates.canInstall)
    return updates.newVersion
      ? `Версия ${updates.newVersion} готова к установке`
      : "Обновление готово к установке";
  if (updates.newVersion) return `Доступна версия ${updates.newVersion}`;
  if (updates.state === "checking") return "Проверяем…";
  if (updates.state === "error") return "Не удалось проверить обновления";
  return "Установлена последняя версия";
}
