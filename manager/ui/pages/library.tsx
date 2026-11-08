import { signal, Text, View } from "nsh";
import {
  Card,
  CardRow,
  PillButton,
  SectionLabel,
  SettingsPage,
  StatusDot,
  TextInput,
} from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { ManagerState } from "../state";

/** The Engine of this branch keeps no installed-app catalog, and the Start-Menu
 * index it does keep is not one: it names programs on the system, not apps
 * Mundus installed. So the page says that much and offers nothing to press —
 * a button that opens what it cannot name is worse than no button. */
export function AppsPage(props: PageProps) {
  return (
    <SettingsPage colors={props.colors} title="Приложения">
      <View>
        <SectionLabel colors={props.colors}>Установленные</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="apps"
            iconFill={props.colors.raised}
            accent={props.colors.muted}
            title="Пока ничего не установлено"
            subtitle="Engine не сообщает об установленных приложениях"
            last
          />
        </Card>
      </View>
    </SettingsPage>
  );
}

export function ModelsPage(props: PageProps) {
  const models = () => props.state().models;
  const stateOf = (model: ManagerState["models"][number]) => {
    if (model.downloading) return `Загружается · ${Math.round(model.percent)}%`;
    if (model.downloaded)
      return model.selected ? `${model.size} · выбрана` : `${model.size} · загружена`;
    return `${model.size} · не загружена`;
  };
  return (
    <SettingsPage colors={props.colors} title="Модели">
      <View>
        <SectionLabel colors={props.colors}>Локальные модели</SectionLabel>
        <Card colors={props.colors}>
          {models().length ? (
            models().map((model, index) => (
              <CardRow
                key={model.id}
                colors={props.colors}
                icon="chip"
                iconFill={props.colors.raised}
                accent={props.colors.muted}
                title={model.name}
                subtitle={stateOf(model)}
                last={index === models().length - 1}
                right={
                  <View class="flex-row items-center gap-2">
                    {model.downloading ? (
                      <Text
                        style={{ fontSize: 12.5, color: props.colors.muted }}
                      >{`${Math.round(model.percent)}%`}</Text>
                    ) : model.downloaded ? (
                      <>
                        {model.selected ? <StatusDot colors={props.colors} /> : null}
                        {!model.selected ? (
                          <PillButton
                            colors={props.colors}
                            tone="accent"
                            label="Выбрать"
                            onPress={() => props.onCommand("models.select", { id: model.id })}
                          />
                        ) : null}
                        <PillButton
                          colors={props.colors}
                          label="Удалить"
                          onPress={() => props.onCommand("models.delete", { id: model.id })}
                        />
                      </>
                    ) : (
                      <PillButton
                        colors={props.colors}
                        tone="accent"
                        label="Скачать"
                        onPress={() => props.onCommand("models.download", { id: model.id })}
                      />
                    )}
                  </View>
                }
              />
            ))
          ) : (
            <CardRow
              colors={props.colors}
              title="Список пуст"
              subtitle="Engine не предложил ни одной модели"
              last
            />
          )}
        </Card>
      </View>
      <Text style={{ fontSize: 12, color: props.colors.faint }}>
        Модели распознавания живут на этом компьютере: звук не уходит в сеть.
      </Text>
    </SettingsPage>
  );
}

/// The dictation key is written once and never read back: the Engine answers
/// whether it has one, not what it is.
const draft = signal("");
const verifyNote = signal("");

export function KeysPage(props: PageProps) {
  const keys = () => props.state().keys;
  const save = () => {
    const value = draft();
    if (!value.trim()) return;
    props.onCommand("keys.save", { value });
    draft.set("");
    verifyNote.set("Ключ отправлен в Engine");
  };
  return (
    <SettingsPage colors={props.colors} title="Ключи">
      <View>
        <SectionLabel colors={props.colors}>Диктовка</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            icon="key"
            iconFill={props.colors.raised}
            accent={keys().hasApiKey ? props.colors.success : props.colors.muted}
            title="Ключ распознавания"
            subtitle={
              keys().hasApiKey
                ? "Сохранён в Engine"
                : "Не сохранён — работает только локальная модель"
            }
            right={
              <StatusDot colors={props.colors} tone={keys().hasApiKey ? "success" : "muted"} />
            }
          />
          <CardRow colors={props.colors} title="Движок" subtitle={keys().engine || "локальный"} />
          <CardRow
            colors={props.colors}
            title="Язык"
            subtitle={keys().language || "не выбран"}
            last={!verifyNote()}
          />
          {verifyNote() ? (
            <CardRow colors={props.colors} title="Проверка" subtitle={verifyNote()} last />
          ) : null}
        </Card>
      </View>

      <View>
        <SectionLabel colors={props.colors}>
          {keys().hasApiKey ? "Заменить ключ" : "Добавить ключ"}
        </SectionLabel>
        <Card colors={props.colors} padded>
          <View style={{ gap: 10 }}>
            <TextInput
              colors={props.colors}
              value={draft()}
              placeholder="Вставьте ключ"
              accessibilityLabel="Ключ диктовки"
              onChange={(value) => {
                draft.set(value);
                verifyNote.set("");
              }}
              onSubmit={save}
            />
            <View class="flex-row items-center justify-end gap-2">
              <PillButton
                colors={props.colors}
                label="Проверить"
                onPress={() => props.onCommand("keys.verify", { value: draft() })}
              />
              <PillButton colors={props.colors} tone="accent" label="Сохранить" onPress={save} />
            </View>
          </View>
        </Card>
      </View>
      <Text style={{ fontSize: 12, color: props.colors.faint }}>
        Ключ хранится в связке ключей Engine. Manager его не читает и нигде не показывает.
      </Text>
    </SettingsPage>
  );
}
