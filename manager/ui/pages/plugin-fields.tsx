import { Scroll, signal, Text, View } from "nsh";
import { FieldRow, PillButton, SearchField } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import type { SettingValue } from "../state";
import { Modal } from "../modal";

/** A choice as a row of buttons: one is on, the rest ask to be. */
export function ChoiceRow(props: {
  label: string;
  options: { value: SettingValue; label: string }[];
  value: SettingValue | undefined;
  colors: ApplicationTheme;
  last: boolean;
  onPick: (value: SettingValue) => void;
}) {
  const { colors, value } = props;
  return (
    <FieldRow colors={colors} label={props.label} last={props.last} labelWidth={LABEL_WIDTH}>
      <View class="flex-row flex-wrap items-center" style={{ gap: 6 }}>
        {props.options.map((option) => (
          <PillButton
            key={option.label}
            colors={colors}
            tone={same(option.value, value) ? "accent" : "neutral"}
            label={option.label}
            onPress={() => props.onPick(option.value)}
          />
        ))}
      </View>
    </FieldRow>
  );
}

/** Two stored values are the same choice: `"ru"` with `"ru"`, `600000` with
 * `600000`, `null` with `null` — not `"5"` with `5`. */
export function same(option: SettingValue | undefined, value: SettingValue | undefined): boolean {
  return JSON.stringify(option ?? null) === JSON.stringify(value ?? null);
}

/** The open picker: which field, and what has been typed into it. One at a
 * time, so the page never stacks two lists on top of each other. */
export const picking = signal<{ key: string; query: string }>({ key: "", query: "" });

/** A long choice (a hundred languages) as one current value and a searchable
 * list behind it — a row of a hundred buttons is not a choice, it is a wall. */
export function PickerRow(props: {
  fieldKey: string;
  label: string;
  options: { value: SettingValue; label: string }[];
  value: SettingValue | undefined;
  colors: ApplicationTheme;
  last: boolean;
}) {
  const { colors } = props;
  const selected = props.options.find((option) => same(option.value, props.value));
  return (
    <FieldRow colors={colors} label={props.label} last={props.last} labelWidth={LABEL_WIDTH}>
      {/* A pill hugs its text: the field column is a stretch column, so the
          trigger has to say so itself. */}
      <View class="flex-row">
        <PillButton
          colors={colors}
          tone="neutral"
          label={selected?.label ?? "Не выбрано"}
          onPress={() => picking.set({ key: props.fieldKey, query: "" })}
        />
      </View>
    </FieldRow>
  );
}

/** The list a `PickerRow` opens: drawn beside the page, not inside the row, so
 * it covers the window the way every other modal of the Manager does. */
export function PickerModal(props: {
  title: string;
  options: { value: SettingValue; label: string }[];
  value: SettingValue | undefined;
  colors: ApplicationTheme;
  onPick: (value: SettingValue) => void;
}) {
  const { colors } = props;
  const close = () => picking.set({ key: "", query: "" });
  const matching = () => {
    const needle = picking().query.trim().toLowerCase();
    if (!needle) return props.options;
    return props.options.filter((option) => option.label.toLowerCase().includes(needle));
  };
  return (
    <Modal title={props.title} colors={colors} onDismiss={close} width={380}>
      <SearchField
        colors={colors}
        value={picking().query}
        placeholder="Поиск"
        onChange={(next) => picking.set({ key: picking().key, query: next })}
      />
      <Scroll class="min-h-0" style={{ height: 320 }}>
        {matching().map((option) => (
          <View
            key={option.label}
            role="button"
            accessibilityLabel={option.label}
            onPress={() => {
              close();
              props.onPick(option.value);
            }}
            class="h-9 flex-row items-center justify-between rounded-md px-2"
            hoverStyle={{ backgroundColor: colors.background }}
          >
            <Text class="text-[13px]" style={{ color: colors.foreground }}>
              {option.label}
            </Text>
            {same(option.value, props.value) ? (
              <Text class="text-[13px]" style={{ color: colors.accent }}>
                ✓
              </Text>
            ) : null}
          </View>
        ))}
      </Scroll>
    </Modal>
  );
}

/** The label column: wide enough for a two-word setting name, so a field never
 * wraps into the control's row (`Подсказка для расшифровки` is the longest). */
export const LABEL_WIDTH = 210;

/** Above this many options a choice is a list, not a row of buttons. */
export const CHIPS = 6;
