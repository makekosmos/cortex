import { Text, View } from "nsh";
import { Card, CardRow, PillButton, SectionLabel, Switch } from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import { minutes } from "../state";
import { Page } from "../page";

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

export function StoragePage(props: PageProps) {
  const state = () => props.state();
  const backups = () => state().backups;
  return (
    <Page colors={props.colors} title="Хранилище">
      <View>
        <SectionLabel colors={props.colors}>Данные</SectionLabel>
        <Card colors={props.colors}>
          <CardRow colors={props.colors} title="Папка данных" subtitle={state().storage.dataDir} />
          <CardRow
            colors={props.colors}
            title="Размер базы"
            subtitle={state().storage.size || "неизвестен"}
            last
          />
        </Card>
      </View>
      <View>
        <SectionLabel colors={props.colors}>Резервные копии</SectionLabel>
        <Card colors={props.colors}>
          {backups().length ? (
            backups().map((backup, index, list) => (
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
              subtitle="Учёт активности выключен или пуст"
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
