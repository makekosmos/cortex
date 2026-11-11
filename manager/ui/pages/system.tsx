import { View } from "nsh";
import { platform } from "os";
import { Card, CardRow, SectionLabel, Switch } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { minutes } from "../state";
import { Page } from "../page";

export function ActivityPage(props: PageProps) {
  const usage = () => props.state().usage;
  return (
    <Page colors={props.colors} title="Активность">
      <View>
        <SectionLabel colors={props.colors}>Экранное время</SectionLabel>
        <Card colors={props.colors}>
          <CardRow colors={props.colors} title="Сегодня" subtitle={minutes(usage().todayMinutes)} />
          <CardRow
            colors={props.colors}
            title="За неделю"
            subtitle={minutes(usage().weekMinutes)}
            last
          />
        </Card>
      </View>
      <View>
        <SectionLabel colors={props.colors}>Чаще всего</SectionLabel>
        <Card colors={props.colors}>
          {usage().top.length ? (
            usage().top.map((program, index) => (
              <CardRow
                key={program.name}
                colors={props.colors}
                title={program.name}
                subtitle={minutes(program.minutes)}
                last={index === usage().top.length - 1}
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              title="Пока нечего показать"
              subtitle={
                platform() === "darwin"
                  ? "Движок пока считает активность только на Windows"
                  : "Учёт активности выключен или пуст"
              }
              last
            />
          )}
        </Card>
      </View>
    </Page>
  );
}

export function PreferencesPage(props: PageProps) {
  const state = () => props.state();
  return (
    <Page colors={props.colors} title="Настройки">
      <View>
        <SectionLabel colors={props.colors}>Общие</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            title="Запуск при входе"
            subtitle="Engine и Manager стартуют вместе с системой"
            right={
              <Switch
                colors={props.colors}
                value={state().settings.autostart}
                accessibilityLabel="Запуск при входе"
                onChange={(value) => props.onCommand("settings.autostart", { value })}
              />
            }
          />
          <CardRow
            colors={props.colors}
            title="Телеметрия"
            subtitle="Общая статистика использования Engine"
            last
            right={
              <Switch
                colors={props.colors}
                value={state().settings.telemetry}
                accessibilityLabel="Телеметрия"
                onChange={(value) => props.onCommand("settings.telemetry", { value })}
              />
            }
          />
        </Card>
      </View>
    </Page>
  );
}
