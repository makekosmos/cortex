import { Scroll, Text, View } from "nsh";
import { titleBar } from "nsh/desktop";
import { settingsMetrics as M } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";

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
          paddingTop: M.pageTop,
          paddingBottom: M.pageBottom,
        }}
      >
        <View
          class="flex-row items-center"
          style={{ height: M.headerHeight, paddingLeft: controls.left }}
          desktop={{ windowDrag: true }}
        >
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
