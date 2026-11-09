import { Scroll, Text, View } from "nsh";
import { Icon, SearchField, settingsMetrics } from "@makekosmos/imago-nsh";
import type { ApplicationTheme, IconName } from "@makekosmos/imago-nsh";
import type { NavRow, NavSection } from "./navigation";

/** The grey the product's own tile wears: almost black, never a colour. */
const HEAD_TILE = "#1a1a1a";

/**
 * One sidebar row. Everything but the product's own row is a plain icon and a
 * label — no tile, no accent: the sidebar is a list of places, not a set of
 * badges, and colour here only competed with the page it opened.
 */
function SidebarRow(props: {
  row: NavRow;
  selected: boolean;
  colors: ApplicationTheme;
  onPress: () => void;
}) {
  const { colors, row, selected } = props;
  const ink = selected ? colors.foreground : colors.muted;
  return (
    <View
      onPress={props.onPress}
      role="button"
      accessibilityLabel={row.label}
      accessibilityState={{ selected }}
      class="flex-row items-center gap-2.5 rounded-[10px]"
      style={{
        height: row.head ? settingsMetrics.logoRowHeight : settingsMetrics.navRowHeight,
        paddingLeft: settingsMetrics.cardPaddingX,
        paddingRight: settingsMetrics.cardPaddingX,
        backgroundColor: selected ? colors.raised : "transparent",
      }}
    >
      {row.head ? (
        <View
          class="items-center justify-center"
          style={{
            width: settingsMetrics.logoTileSize,
            height: settingsMetrics.logoTileSize,
            borderRadius: 10,
            backgroundColor: HEAD_TILE,
            borderWidth: 1,
            borderColor: "rgba(255,255,255,0.10)",
          }}
        >
          <Icon
            name="mundus"
            size={Math.round(settingsMetrics.logoTileSize * 0.56)}
            color="#ffffff"
          />
        </View>
      ) : (
        <View class="items-center justify-center" style={{ width: 18 }}>
          <Icon name={row.icon} size={18} color={ink} />
        </View>
      )}
      <Text
        style={{
          flexGrow: 1,
          fontSize: 14,
          lineHeight: 20,
          fontWeight: row.head ? "600" : "500",
          color: colors.foreground,
        }}
      >
        {row.label}
      </Text>
    </View>
  );
}

/**
 * The Manager's sidebar: the search field under the window's drag band, then
 * the sections. Everything above the field stays part of the band, so dragging
 * the window by the corner above «Поиск настроек» still works.
 */
export function Sidebar(props: {
  colors: ApplicationTheme;
  sections: NavSection[];
  current: string;
  search: string;
  top: number;
  onSearch: (value: string) => void;
  onNavigate: (route: string) => void;
}) {
  const matches = (row: NavRow) =>
    !props.search.trim() || row.label.toLowerCase().includes(props.search.trim().toLowerCase());
  return (
    <View
      class="h-full shrink-0 flex-col"
      style={{
        width: settingsMetrics.sidebarWidth,
        backgroundColor: props.colors.shell,
        paddingTop: props.top,
        paddingBottom: settingsMetrics.sidebarTop,
        paddingLeft: settingsMetrics.sidebarPadding,
        paddingRight: settingsMetrics.sidebarPadding,
      }}
    >
      <View class="min-h-0 flex-1 flex-col" style={{ gap: settingsMetrics.searchGap }}>
        <SearchField
          value={props.search}
          placeholder="Поиск настроек"
          colors={props.colors}
          onChange={props.onSearch}
        />
        <Scroll class="min-h-0 flex-1">
          <View style={{ gap: settingsMetrics.navGroupGap }}>
            {props.sections.map((section) => {
              const rows = section.rows.filter(matches);
              if (!rows.length) return null;
              return (
                <View key={section.id} style={{ gap: settingsMetrics.navEntryGap }}>
                  {rows.map((row) => (
                    <SidebarRow
                      key={row.route}
                      row={row}
                      colors={props.colors}
                      selected={props.current === row.route}
                      onPress={() => props.onNavigate(row.route)}
                    />
                  ))}
                </View>
              );
            })}
          </View>
        </Scroll>
      </View>
    </View>
  );
}

export type { IconName };
