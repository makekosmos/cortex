import { Text, View } from "nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";

/** `FnFn` is a double tap, shown as one block: `Fn + Fn`. */
function hotkeyLabel(value: string): string {
  const text = value.trim();
  if (!text) return "Не задано";
  if (text.includes("+")) {
    return text
      .split("+")
      .map((part) => part.trim())
      .filter(Boolean)
      .join(" + ");
  }
  const half = Math.floor(text.length / 2);
  if (text.length >= 2 && text.length % 2 === 0 && text.slice(0, half) === text.slice(half)) {
    return `${text.slice(0, half)} + ${text.slice(half)}`;
  }
  return text;
}

/** The glyph of each modifier the Engine names in a hotkey string. */
const KEYCAPS = new Map<string, string>([
  ["ctrl", "⌃"],
  ["alt", "⌥"],
  ["shift", "⇧"],
  ["cmd", "⌘"],
  ["fn", "Fn"],
]);

/** A modifier's glyph, or the name itself when it has none. */
function keycap(name: string): string {
  return KEYCAPS.get(name) ?? name;
}

function Keycap(props: { label: string; colors: ApplicationTheme; dim?: boolean }) {
  return (
    <View
      class="items-center justify-center"
      style={{
        minWidth: 26,
        height: 26,
        paddingLeft: 6,
        paddingRight: 6,
        borderRadius: 7,
        backgroundColor: props.dim ? props.colors.card : props.colors.background,
        borderWidth: 1,
        borderColor: props.colors.border,
      }}
    >
      <Text
        style={{
          fontSize: 13,
          color: props.dim ? props.colors.faint : props.colors.foreground,
        }}
      >
        {props.label}
      </Text>
    </View>
  );
}

/**
 * The hotkey picker, the way Raycast shows one: a chip that becomes a small
 * keycap panel while the person presses a combination. Nothing is typed — the
 * Manager's own event tap reads the keys, so the result is the same whether
 * this window has the focus or not.
 */
export function HotkeyRecorder(props: {
  value: string;
  colors: ApplicationTheme;
  capturing: boolean;
  held: string[];
  listenAccess: boolean;
  onStart: () => void;
  onOpenSettings: () => void;
}) {
  if (props.capturing) {
    return (
      <View class="w-full flex-col items-end" style={{ rowGap: 6 }}>
        <View
          class="flex-row items-center"
          style={{
            columnGap: 6,
            padding: 6,
            borderRadius: 10,
            backgroundColor: props.colors.raised,
            borderWidth: 1,
            borderColor: props.colors.accent,
          }}
        >
          {props.held.length ? (
            props.held.map((key) => <Keycap key={key} label={keycap(key)} colors={props.colors} />)
          ) : (
            <Text
              style={{
                fontSize: 12,
                color: props.colors.muted,
                paddingLeft: 6,
                paddingRight: 6,
              }}
            >
              Нажми сочетание
            </Text>
          )}
        </View>
        <Text style={{ fontSize: 11, color: props.colors.muted }}>Esc — отмена</Text>
      </View>
    );
  }
  return (
    <View class="w-full flex-col items-end" style={{ rowGap: 4 }}>
      <Text
        onPress={props.onStart}
        style={{
          fontSize: 12,
          color: props.colors.foreground,
          backgroundColor: props.colors.raised,
          paddingLeft: 10,
          paddingRight: 10,
          paddingTop: 4,
          paddingBottom: 4,
          borderRadius: 8,
          borderWidth: 1,
          borderColor: props.colors.border,
        }}
      >
        {hotkeyLabel(props.value)}
      </Text>
      {props.listenAccess ? null : (
        <Text onPress={props.onOpenSettings} style={{ fontSize: 11, color: props.colors.muted }}>
          Хоткей молчит: нет «Мониторинга ввода» — открыть настройки
        </Text>
      )}
    </View>
  );
}
