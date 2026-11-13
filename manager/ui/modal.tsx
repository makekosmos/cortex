import { Text, View } from "nsh";
import { PillButton } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";

/**
 * The one modal shell of the Manager: a dimmed page, a card with a hairline
 * rule, a title row and a footer row. The backdrop is the card's sibling, not
 * its ancestor — a press inside the card must not close it.
 */
export function Modal(props: {
  title: string;
  colors: ApplicationTheme;
  onDismiss: () => void;
  children: any;
  footer?: any;
  width?: number;
}) {
  return (
    <View class="absolute inset-0 items-center justify-center px-6">
      <View
        onPress={props.onDismiss}
        accessibilityLabel="Закрыть окно"
        role="button"
        class="absolute inset-0"
        style={{ backgroundColor: "#00000059" }}
      />
      <View
        class="z-10 flex-col rounded-[16px] border"
        style={{
          width: props.width ?? 420,
          borderColor: props.colors.border,
          backgroundColor: props.colors.card,
        }}
        role="none"
      >
        <View class="w-full flex-row items-center px-5 pb-3 pt-5" style={{ gap: 6 }}>
          <Text class="flex-1 text-[15px] font-semibold" style={{ color: props.colors.foreground }}>
            {props.title}
          </Text>
          <View
            onPress={props.onDismiss}
            role="button"
            accessibilityLabel="Закрыть карточку"
            class="h-6 w-6 items-center justify-center rounded-md"
            hoverStyle={{ backgroundColor: props.colors.background }}
          >
            <Text class="text-[13px]" style={{ color: props.colors.muted }}>
              ✕
            </Text>
          </View>
        </View>
        <View class="w-full px-5 pb-5" style={{ gap: 10 }}>
          {props.children}
        </View>
        {props.footer ? (
          <View
            class="flex-row items-center justify-end border-t px-5 py-3"
            style={{ borderColor: props.colors.border, gap: 12 }}
          >
            {props.footer}
          </View>
        ) : null}
      </View>
    </View>
  );
}

/** A sentence in a modal: the question the user is answering. */
export function note(colors: ApplicationTheme, text: string) {
  return (
    <Text class="text-[12px] leading-[18px]" style={{ color: colors.muted }}>
      {text}
    </Text>
  );
}

export function Cancel(props: { colors: ApplicationTheme; onPress: () => void; label?: string }) {
  return (
    <PillButton colors={props.colors} label={props.label ?? "Отмена"} onPress={props.onPress} />
  );
}

export function Confirm(props: {
  colors: ApplicationTheme;
  onPress: () => void;
  label: string;
  tone?: "accent" | "danger";
}) {
  return (
    <PillButton
      colors={props.colors}
      tone={props.tone ?? "accent"}
      label={props.label}
      onPress={props.onPress}
    />
  );
}
