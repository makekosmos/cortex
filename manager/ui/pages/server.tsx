import { Text, View } from "nsh";
import { Card, CardRow, FieldRow, PillButton, SectionLabel, Switch } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { productVersion, statusLabel } from "../state";
import { Page } from "../page";
import { updateAction } from "./updates";

/** The page of the reference: what the Engine is doing right now. */
export function ServerPage(props: PageProps) {
  const state = () => props.state();
  const code = state().sync.ownCode;
  const engineRunning = () => state().engine.status === "running";
  const engineBusy = () =>
    state().engine.status === "starting" || state().engine.status === "stopping";
  // The Engine, named and versioned, as its subtitle: the header says what the
  // product is, this row says what runs under it.
  const engineSubtitle = () => {
    const engine = state().engine;
    return engine.version
      ? `${statusLabel(engine.status)} · ${engine.version}`
      : statusLabel(engine.status);
  };
  // The header's one button is the release: download it, or install it once it
  // is ready (green). The Engine's own lifetime lives in the card below.
  const releaseWaiting = () => {
    const updates = state().updates;
    return (
      updates.canInstall ||
      !!updates.newVersion ||
      (updates.percent > 0 && updates.percent < 100) ||
      updates.state === "error"
    );
  };
  return (
    <Page colors={props.colors} title="Сервер">
      <Card colors={props.colors}>
        <CardRow
          colors={props.colors}
          logo
          title={productVersion(state()) ? `Mundus ${productVersion(state())}` : "Mundus"}
          subtitle={
            productVersion(state())
              ? `${statusLabel(state().engine.status)} · канал ${state().engine.channel}`
              : statusLabel(state().engine.status)
          }
          last
          right={
            releaseWaiting()
              ? updateAction({ ...props, updates: state().updates })
              : undefined
          }
        />
      </Card>

      <View>
        <SectionLabel colors={props.colors}>Движок</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="pulse"
            title="Engine"
            subtitle={engineSubtitle()}
            right={
              <View class="flex-row items-center gap-2">
                <PillButton
                  colors={props.colors}
                  tone={engineRunning() ? "danger" : "accent"}
                  label={engineRunning() ? "Остановить" : "Запустить"}
                  disabled={engineBusy()}
                  onPress={() =>
                    props.onCommand(engineRunning() ? "server.stop" : "server.start")
                  }
                />
                <PillButton
                  colors={props.colors}
                  label="Перезапустить"
                  disabled={engineBusy()}
                  onPress={() => props.onCommand("server.restart")}
                />
              </View>
            }
          />
          <CardRow
            colors={props.colors}
            icon="display"
            title="Запуск при входе"
            subtitle="Движок поднимается сам при входе в систему — диктовка работает без Manager"
            last
            right={
              <Switch
                colors={props.colors}
                value={state().settings.autostart}
                accessibilityLabel="Запуск при входе"
                onChange={(value) => props.onCommand("settings.autostart", { value })}
              />
            }
          />
        </Card>
      </View>

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
