import { Text, View } from "nsh";
import { Card, CardRow, PillButton, SectionLabel, StatusDot } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { Page } from "../page";

export function DevicesPage(props: PageProps) {
  const state = () => props.state();
  const peers = () => state().sync.peers;
  return (
    <Page colors={props.colors} title="Девайсы">
      <View>
        <SectionLabel colors={props.colors}>Этот компьютер</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="monitor"
            iconFill={props.colors.raised}
            accent={props.colors.accent}
            title={state().client.name}
            subtitle={
              state().sync.running
                ? "Текущее устройство · синхронизация включена"
                : "Текущее устройство"
            }
            right={
              <StatusDot colors={props.colors} tone={state().sync.running ? "success" : "muted"} />
            }
            last
          />
        </Card>
      </View>
      <View>
        <SectionLabel colors={props.colors}>Связанные</SectionLabel>
        <Card colors={props.colors}>
          {peers().length ? (
            peers().map((device, index, list) => (
              <CardRow
                key={device.id}
                colors={props.colors}
                icon="monitor"
                iconFill={props.colors.raised}
                title={device.name}
                subtitle={device.status === "online" ? "Онлайн" : "Не в сети"}
                last={index === list.length - 1}
                right={
                  <View class="flex-row items-center gap-2">
                    <StatusDot
                      colors={props.colors}
                      tone={device.status === "online" ? "success" : "muted"}
                    />
                    <PillButton
                      colors={props.colors}
                      tone="danger"
                      label="Отключить"
                      onPress={() => props.onCommand("device.unlink", { id: device.id })}
                    />
                  </View>
                }
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              title="Пока никого"
              subtitle="Подключите второе устройство, чтобы делиться задачами и заметками"
              last
            />
          )}
        </Card>
      </View>
      <View>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="plus"
            iconFill={props.colors.raised}
            title="Подключить устройство"
            subtitle="Введите код, который показывает другое устройство"
            last
            right={
              <PillButton
                colors={props.colors}
                tone="accent"
                label="Подключить"
                onPress={() => props.onCommand("connect.open")}
              />
            }
          />
        </Card>
      </View>
    </Page>
  );
}

/** Connectors are the Engine's (`integrations.list`): the Manager shows what it
 * reports and can ask for a sync. Which services exist is the Engine's catalog,
 * not a list the Manager keeps. */
export function IntegrationsPage(props: PageProps) {
  const connectors = () => props.state().connectors;
  return (
    <Page colors={props.colors} title="Интеграции">
      <View>
        <SectionLabel colors={props.colors}>Коннекторы</SectionLabel>
        <Card colors={props.colors}>
          {connectors().length ? (
            connectors().map((connector, index, list) => (
              <CardRow
                key={connector.id}
                colors={props.colors}
                icon="plug"
                iconFill={props.colors.raised}
                accent={connector.connected ? props.colors.success : props.colors.muted}
                title={connector.name}
                subtitle={
                  connector.detail || (connector.connected ? "Подключено" : "Не подключено")
                }
                last={index === list.length - 1}
                right={
                  <StatusDot
                    colors={props.colors}
                    tone={connector.connected ? "success" : "muted"}
                  />
                }
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              title="Коннекторов нет"
              subtitle="Engine не сообщает о подключённых сервисах"
              last
            />
          )}
        </Card>
      </View>
      <View>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="key"
            iconFill={props.colors.raised}
            title="Доступы и ключи"
            subtitle="Диктовка и распознавание речи"
            right={
              <PillButton
                colors={props.colors}
                label="Открыть"
                onPress={() => props.onCommand("route", { route: "keys" })}
              />
            }
          />
          <CardRow
            colors={props.colors}
            icon="pulse"
            iconFill={props.colors.raised}
            title="Синхронизировать сейчас"
            subtitle="Забрать свежие данные из подключённых сервисов"
            last
            right={
              <PillButton
                colors={props.colors}
                tone="accent"
                label="Синхронизировать"
                onPress={() => props.onCommand("connectors.syncNow")}
              />
            }
          />
        </Card>
      </View>
    </Page>
  );
}

/**
 * The Manager hosts no browser of its own and the Engine exposes no browser
 * settings, so the page states what is true and offers nothing to press: a
 * control that never reaches the Engine is worse than no control.
 */
export function BrowserPage(props: PageProps) {
  return (
    <Page colors={props.colors} title="Браузер">
      <View>
        <SectionLabel colors={props.colors}>Ссылки</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="external"
            iconFill={props.colors.raised}
            title="Системный браузер"
            subtitle="Mundus не держит свой: ссылки открывает тот браузер, который выбран в системе"
            last
            right={<Text style={{ fontSize: 12.5, color: props.colors.muted }}>Системный</Text>}
          />
        </Card>
      </View>
    </Page>
  );
}
