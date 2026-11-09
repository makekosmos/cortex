import { Text, View } from "nsh";
import { Card, CardRow, FieldRow, PillButton, SectionLabel } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { productVersion, statusLabel } from "../state";
import type { ManagerState } from "../state";
import { Page } from "../page";

/** The one green pill in the Manager: a version is ready to be taken. The kit
 * has no green tone — its pills are accent, danger or neutral — so the press
 * that updates the product is drawn here, in the status colour the theme
 * already has for "this is good". */
function UpdatePill(props: { colors: ApplicationTheme; label: string; onPress: () => void }) {
  return (
    <View
      onPress={props.onPress}
      role="button"
      accessibilityLabel={props.label}
      hoverStyle={{ opacity: 0.9 }}
      class="items-center justify-center rounded-full"
      style={{
        paddingLeft: 10,
        paddingRight: 10,
        paddingTop: 4,
        paddingBottom: 4,
        backgroundColor: props.colors.success,
      }}
    >
      <Text
        style={{ fontSize: 13, lineHeight: 15, fontWeight: "600", color: props.colors.background }}
      >
        {props.label}
      </Text>
    </View>
  );
}

/** True while there is a version to take: the row's button becomes «Обновить». */
function hasUpdate(updates: ManagerState["updates"]): boolean {
  return Boolean(updates.newVersion) || updates.canInstall;
}

/** What the update is doing, in one line under «Обновления». */
function updateSubtitle(updates: ManagerState["updates"]): string {
  if (updates.percent > 0 && updates.percent < 100)
    return `Загружается · ${Math.round(updates.percent)}%`;
  if (updates.canInstall) return `Версия ${updates.newVersion} готова к установке`;
  if (updates.newVersion) return `Доступна версия ${updates.newVersion}`;
  if (updates.state === "checking") return "Проверяем…";
  if (updates.state === "error") return "Не удалось проверить обновления";
  return "Установлена последняя версия";
}

/**
 * Mundus itself: whether it runs, what can be updated, and what it is connected
 * to. The version lives in the first row — an update is a fact about this
 * computer, not a page of its own.
 */
export function ServerPage(props: PageProps) {
  const state = () => props.state();
  const running = state().engine.status === "running";
  const busy = state().engine.status === "starting" || state().engine.status === "stopping";
  const updates = () => state().updates;
  const update = () => hasUpdate(updates());
  const version = productVersion(state());
  const code = state().sync.ownCode;
  const subtitle = [statusLabel(state().engine.status), version].filter(Boolean).join(" · ");
  return (
    <Page colors={props.colors} title="Mundus">
      <Card colors={props.colors}>
        <CardRow
          colors={props.colors}
          icon="mundus"
          iconFill="#1a1a1a"
          title="Mundus"
          subtitle={subtitle}
          right={
            // One button for both answers: a version to take comes first —
            // pressing it installs, and only then can the Engine be stopped.
            update() ? (
              <UpdatePill
                colors={props.colors}
                label="Обновить"
                onPress={() => props.onCommand("updates.install")}
              />
            ) : (
              <PillButton
                colors={props.colors}
                tone={running ? "danger" : "accent"}
                label={running ? "Остановить" : "Запустить"}
                disabled={busy}
                onPress={() => props.onCommand(running ? "server.stop" : "server.start")}
              />
            )
          }
        />
        {/* The update is this computer's fact, not a page of its own: it reads
            here, under the version it would replace. */}
        <CardRow
          colors={props.colors}
          title="Обновления"
          subtitle={updateSubtitle(updates())}
          last
          right={
            update() ? (
              updates().canInstall ? null : (
                <PillButton
                  colors={props.colors}
                  tone="accent"
                  label="Скачать"
                  onPress={() => props.onCommand("updates.download")}
                />
              )
            ) : (
              <PillButton
                colors={props.colors}
                label="Проверить"
                onPress={() => props.onCommand("updates.check")}
              />
            )
          }
        />
      </Card>

      <View>
        <SectionLabel colors={props.colors}>Коннектор</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="globe"
            title="Подключить другие устройства"
            subtitle={state().sync.running ? "Синхронизация включена" : "Синхронизация выключена"}
            right={
              <PillButton
                colors={props.colors}
                tone="accent"
                label="Подключить"
                onPress={() => props.onCommand("connect.open")}
              />
            }
          />
          <CardRow
            colors={props.colors}
            icon="key"
            title="Код этого компьютера"
            subtitle={
              code ? "Введите его на другом устройстве" : "Покажите код тому, кого подключаете"
            }
            right={
              <PillButton
                colors={props.colors}
                label={code ? "Скрыть" : "Показать"}
                onPress={() => props.onCommand(code ? "pairing.hide" : "pairing.code")}
              />
            }
          />
          {code ? (
            <FieldRow colors={props.colors} label="Код">
              <Text style={{ fontSize: 11.5, color: props.colors.muted }}>{code}</Text>
            </FieldRow>
          ) : null}
          <CardRow
            colors={props.colors}
            icon="monitor"
            title="Девайсов подключено"
            subtitle={
              state().sync.peers.length ? `${state().sync.peers.length} в списке` : "Пока ни одного"
            }
            last
            right={
              <PillButton
                colors={props.colors}
                label="Открыть"
                onPress={() => props.onCommand("route", { route: "devices" })}
              />
            }
          />
        </Card>
      </View>

      <View>
        <SectionLabel colors={props.colors}>Компьютер</SectionLabel>
        <Card colors={props.colors}>
          <FieldRow colors={props.colors} label="Имя">
            <Text style={{ fontSize: 13, color: props.colors.foreground }}>
              {state().client.name}
            </Text>
          </FieldRow>
          <CardRow
            colors={props.colors}
            title="Порт Engine"
            subtitle="Из engine.lock.json того Engine, что отвечает"
            last
            right={
              <Text style={{ fontSize: 12.5, color: props.colors.muted }}>
                {state().engine.port || "—"}
              </Text>
            }
          />
        </Card>
      </View>
    </Page>
  );
}
