import { Scroll, Text, View } from "nsh";
import { titleBar } from "nsh/desktop";
import { Icon, settingsMetrics as M } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import { captionStrip, headerHeight } from "./chrome";
import { dragBinding } from "./host";
import { back, canBack, canForward, forward } from "./history";

/** One half of the anchors' pill: a chevron, dimmed when there is no step. */
function Anchor(props: {
  direction: "back" | "forward";
  enabled: boolean;
  colors: ApplicationTheme;
  onPress: () => void;
}) {
  return (
    <View
      onPress={() => (props.enabled ? props.onPress() : undefined)}
      role="button"
      accessibilityLabel={props.direction === "back" ? "Назад" : "Вперёд"}
      class="items-center justify-center"
      style={{
        width: 34,
        height: 30,
        opacity: props.enabled ? 1 : 0.32,
      }}
    >
      {/* The kit's chevron points right; the back arrow is the same glyph
          turned half round: the shape is symmetric about its middle. */}
      <View style={props.direction === "back" ? { rotate: 180 } : undefined}>
        <Icon name="chevron" size={15} color={props.colors.foreground} />
      </View>
    </View>
  );
}

/** Back and forward, in one pill: the two halves share a fill and a radius. */
function Anchors(props: { colors: ApplicationTheme }) {
  return (
    <View
      class="flex-row items-center"
      style={{
        height: 30,
        borderRadius: 8,
        // The pill clips: the halves stay square inside it, so only the outer
        // corners are round.
        overflow: "hidden",
        backgroundColor: props.colors.raised,
        marginRight: 12,
      }}
    >
      <Anchor direction="back" enabled={canBack()} colors={props.colors} onPress={back} />
      <View style={{ width: 1, height: 18, backgroundColor: props.colors.border }} />
      <Anchor direction="forward" enabled={canForward()} colors={props.colors} onPress={forward} />
    </View>
  );
}

/**
 * The frame every page sits in: the scroll area, its gutter, the heading row the
 * window is dragged by, and the column the sections stack in.
 *
 * The kit's `SettingsPage` does the same but caps the column at 1180 points,
 * which leaves a strip of bare background on a wide window. A Manager window is
 * the size the person gave it, so the cards take all of it.
 */
export function Page(props: {
  colors: ApplicationTheme;
  title?: string;
  actions?: any;
  children: any;
}) {
  const controls = titleBar();
  return (
    <Scroll class="min-h-0 flex-1" style={{ backgroundColor: props.colors.background }}>
      <View
        class="w-full"
        style={{
          paddingLeft: M.pageGutter,
          paddingRight: M.pageGutter,
          paddingTop: 0,
          paddingBottom: M.pageBottom,
        }}
      >
        {/* The heading is the window's title row — the band the caption buttons
            are in — and it stops where those buttons start: this row spans the
            whole column, so without the reserve a press on a button would land
            in a drag area and the button would never see it. A press here moves
            the window, which only the shell can do (`dragBinding`). */}
        <View
          class="flex-row items-center"
          style={{
            height: headerHeight(),
            paddingLeft: controls.left,
            marginRight: captionStrip(),
          }}
          {...dragBinding()}
        >
          <Anchors colors={props.colors} />
          {props.title ? (
            <Text
              style={{
                fontSize: 13,
                lineHeight: 19,
                fontWeight: "500",
                color: props.colors.foreground,
              }}
            >
              {props.title}
            </Text>
          ) : null}
          {props.actions ?? null}
          <View class="flex-1" />
        </View>
        <View style={{ gap: M.sectionGap, marginTop: M.headerGap }}>{props.children}</View>
      </View>
    </Scroll>
  );
}
