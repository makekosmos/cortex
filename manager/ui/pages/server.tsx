import { Text, View } from "nsh";
import {
  Card,
  CardRow,
  FieldRow,
  PillButton,
  SectionLabel,
  SettingsPage,
  Switch,
  TextInput,
} from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { statusLabel } from "../state";

/** The page of the reference: what the Engine is doing right now. */
export function ServerPage(props: PageProps) {
  const state = () => props.state();
  const running = state().engine.status === "running";
  const busy = state().engine.status === "starting" || state().engine.status === "stopping";
  const code = state().sync.ownCode;
  return (
    <SettingsPage colors={props.colors} title="Сервер">
      <Card colors={props.colors}>
        <CardRow
          colors={props.colors}
          logo
          title="Мундус"
          subtitle={statusLabel(state().engine.status)}
          last
          right={
            <PillButton
              colors={props.colors}
              tone={running ? "danger" : "accent"}
              label={running ? "Остановить" : "Запустить"}
              disabled={busy}
              onPress={() => props.onCommand(running ? "server.stop" : "server.start")}
            />
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
                onPress={() => props.onCommand("pairing.code")}
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
        <SectionLabel colors={props.colors}>Информация о Клиенте</SectionLabel>
        <Card colors={props.colors}>
          <FieldRow colors={props.colors} label="Название">
            <TextInput
              colors={props.colors}
              value={state().client.name}
              placeholder="Имя компьютера"
              accessibilityLabel="Название клиента"
              onChange={(value) => props.onCommand("client.name", { value })}
            />
          </FieldRow>
          <CardRow
            colors={props.colors}
            title="Удаленное управление"
            subtitle={state().client.remoteControl ? "Работает" : "Выключено"}
            last
            right={
              <Switch
                colors={props.colors}
                value={state().client.remoteControl}
                accessibilityLabel="Удаленное управление"
                onChange={(value) => props.onCommand("client.remoteControl", { value })}
              />
            }
          />
        </Card>
      </View>
    </SettingsPage>
  );
}
