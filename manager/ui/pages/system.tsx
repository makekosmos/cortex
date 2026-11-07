import { Text, View } from "nsh";
import {
  Card,
  CardRow,
  FieldRow,
  PillButton,
  SectionLabel,
  SettingsPage,
  StatusDot,
  Switch,
} from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { ManagerState } from "../state";
import { minutes, productVersion, statusLabel } from "../state";

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

/** The update card's one button: check, download, or install. Nothing is
 * downloaded without this press. */
function UpdateAction(props: PageProps & { updates: ManagerState["updates"] }) {
  const { updates } = props;
  if (updates.percent > 0 && updates.percent < 100) {
    return (
      <Text
        style={{ fontSize: 12.5, color: props.colors.muted }}
      >{`${Math.round(updates.percent)}%`}</Text>
    );
  }
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

function updateSubtitle(updates: ManagerState["updates"]): string {
  if (updates.percent > 0 && updates.percent < 100)
    return `Загружается · ${Math.round(updates.percent)}%`;
  if (updates.canInstall) return `Версия ${updates.newVersion} готова к установке`;
  if (updates.newVersion) return `Доступна версия ${updates.newVersion}`;
  if (updates.state === "checking") return "Проверяем…";
  if (updates.state === "error") return "Не удалось проверить обновления";
  return "Установлена последняя версия";
}

export function AboutPage(props: PageProps) {
  const state = () => props.state();
  const version = productVersion(state());
  return (
    <SettingsPage colors={props.colors} title="О приложении">
      <Card colors={props.colors}>
        <CardRow
          colors={props.colors}
          icon="server"
          iconFill={props.colors.raised}
          accent={props.colors.accent}
          title={version ? `Mundus ${version}` : "Mundus"}
          subtitle={
            version
              ? `канал ${state().engine.channel || state().product.channel}`
              : statusLabel(state().engine.status)
          }
          right={
            <StatusDot
              colors={props.colors}
              tone={state().engine.status === "running" ? "success" : "muted"}
            />
          }
        />
        <CardRow
          colors={props.colors}
          title="Engine"
          subtitle={
            state().engine.version
              ? `${statusLabel(state().engine.status)} · ${state().engine.version}`
              : statusLabel(state().engine.status)
          }
          right={
            <Text style={{ fontSize: 12.5, color: props.colors.muted }}>
              порт {state().engine.port}
            </Text>
          }
        />
        <CardRow
          colors={props.colors}
          title="Обновления"
          subtitle={updateSubtitle(state().updates)}
          right={<UpdateAction {...props} updates={state().updates} />}
        />
        <CardRow
          colors={props.colors}
          title="Mundus Manager"
          subtitle={state().product.bundleId ?? ""}
          right={<Text style={{ fontSize: 12.5, color: props.colors.muted }}>менеджер</Text>}
          last
        />
      </Card>
      <Text style={{ fontSize: 12, color: props.colors.faint }}>© Yoso Industries 2025–2026</Text>
    </SettingsPage>
  );
}

export function StoragePage(props: PageProps) {
  const state = () => props.state();
  const backups = () => state().backups;
  return (
    <SettingsPage colors={props.colors} title="Хранилище">
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
    </SettingsPage>
  );
}

export function ActivityPage(props: PageProps) {
  const usage = () => props.state().usage;
  return (
    <SettingsPage colors={props.colors} title="Активность">
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
    </SettingsPage>
  );
}

export function PreferencesPage(props: PageProps) {
  const state = () => props.state();
  return (
    <SettingsPage colors={props.colors} title="Настройки">
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
          <FieldRow colors={props.colors} label="Язык">
            <Text style={{ fontSize: 13, color: props.colors.foreground }}>
              {state().keys.language || state().settings.language}
            </Text>
          </FieldRow>
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
    </SettingsPage>
  );
}

export function DeveloperPage(props: PageProps) {
  return (
    <SettingsPage colors={props.colors} title="Разработчикам">
      <View>
        <SectionLabel colors={props.colors}>Инструменты</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="pulse"
            iconFill={props.colors.raised}
            title="Счётчик кадров"
            subtitle="Оверлей FPS — в разработке"
            right={<Text style={{ fontSize: 12.5, color: props.colors.faint }}>скоро</Text>}
          />
          <CardRow
            colors={props.colors}
            icon="code"
            iconFill={props.colors.raised}
            title="Контракт Engine"
            subtitle="Операции /v1/rpc этого Engine"
            right={<Text style={{ fontSize: 12.5, color: props.colors.faint }}>скоро</Text>}
          />
          <CardRow
            colors={props.colors}
            icon="database"
            iconFill={props.colors.raised}
            title="Журналы Engine"
            subtitle="mundus-engine.<date>.log"
            right={<Text style={{ fontSize: 12.5, color: props.colors.faint }}>скоро</Text>}
            last
          />
        </Card>
      </View>
    </SettingsPage>
  );
}
