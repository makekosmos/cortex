import { Scroll, Text, View } from "nsh";
import { Icon, SearchField, settingsMetrics as M } from "@makekosmos/imago-nsh";
import type { ApplicationTheme, IconName, NavGroup } from "@makekosmos/imago-nsh";

/**
 * A colour moved toward black or white.
 *
 * The kit's tiles are gradient plates with a white sheen — the colour language
 * of a launcher. Mundus wears one flat plate per row, a shade darker than the
 * page, so the glyph carries the meaning and the sidebar stays quiet. NSH
 * paints a gradient, so the plate is a single solid colour here.
 */
function shade(hex: string, amount: number, toward: "black" | "white" = "black"): string {
  const parse = (value: string) => {
    const clean = value.replace("#", "");
    return [0, 2, 4].map((index) => parseInt(clean.slice(index, index + 2), 16));
  };
  const from = parse(hex);
  const target = toward === "black" ? [0, 0, 0] : [255, 255, 255];
  const channel = (index: number) => {
    const value = Math.round(from[index] + (target[index] - from[index]) * amount);
    return Math.max(0, Math.min(255, value)).toString(16).padStart(2, "0");
  };
  return `#${[0, 1, 2].map(channel).join("")}`;
}

/** The sidebar's own rhythm: roomier than the kit's, which packs rows tight. */
const GAP = {
  padX: 12,
  rowPadX: 12,
  rowHeight: 36,
  logoRowHeight: 48,
  searchGap: 10,
  groupGap: 14,
  entryGap: 2,
  tileLabel: 10,
};

/** The tile is smaller than the kit's: a plate, not a badge. */
const TILE = 22;

/** The plate every row's glyph sits on: the page colour, a shade deeper. */
export function plateColor(colors: ApplicationTheme): string {
  return shade(colors.background, 0.22);
}

/** The Mundus mark in a pill of its own — the running server's avatar. */
function MarkTile(props: { size?: number }) {
  const size = props.size ?? M.logoTileSize;
  return (
    <View
      class="items-center justify-center"
      style={{
        width: size,
        height: size,
        borderRadius: size / 2,
        backgroundImage: "linear-gradient(135deg, #93aeea 0%, #5b83d6 34%, #359ecb 100%)",
        borderWidth: 1,
        borderColor: "rgba(255,255,255,0.22)",
      }}
    >
      <Icon name="mundus" size={Math.round(size * 0.56)} color="#ffffff" />
    </View>
  );
}

/** One glyph on a flat plate: no sheen, no hue, the page's own colour. */
function SidebarTile(props: { icon: IconName; colors: ApplicationTheme }) {
  return (
    <View
      class="items-center justify-center"
      style={{
        width: TILE,
        height: TILE,
        borderRadius: 6,
        backgroundColor: plateColor(props.colors),
        // A hairline, not a gloss: the plate keeps one colour and still reads as a
        // plate on a background a shade lighter.
        borderWidth: 1,
        borderColor: "rgba(255,255,255,0.045)",
      }}
    >
      <Icon name={props.icon} size={Math.round(TILE * 0.7)} color={props.colors.foreground} />
    </View>
  );
}

function SidebarRow(props: {
  label: string;
  icon: IconName;
  /** A second line under the label: the server says whether it runs. */
  caption?: string;
  logo?: boolean;
  selected: boolean;
  colors: ApplicationTheme;
  onPress: () => void;
}) {
  return (
    <View
      onPress={props.onPress}
      role="button"
      accessibilityLabel={props.label}
      accessibilityState={{ selected: props.selected }}
      class="flex-row items-center rounded-[10px]"
      style={{
        height: props.caption ? GAP.logoRowHeight : GAP.rowHeight,
        gap: GAP.tileLabel,
        paddingLeft: GAP.rowPadX,
        paddingRight: GAP.rowPadX,
        backgroundColor: props.selected ? props.colors.raised : "transparent",
      }}
    >
      <View style={{ marginTop: props.caption || props.logo ? 0 : -1 }}>
        {props.logo ? <MarkTile /> : <SidebarTile icon={props.icon} colors={props.colors} />}
      </View>
      <View class="flex-1" style={{ gap: 2 }}>
        <Text
          style={{
            fontSize: 14,
            fontWeight: "500",
            lineHeight: 20,
            color: props.colors.foreground,
          }}
        >
          {props.label}
        </Text>
        {props.caption ? (
          <Text
            style={{
              fontSize: 12,
              lineHeight: 16,
              fontWeight: "500",
              color: props.colors.muted,
            }}
          >
            {props.caption}
          </Text>
        ) : null}
      </View>
    </View>
  );
}

/**
 * The sidebar. Its own column, so the window can keep the search field clear of
 * the traffic lights: macOS puts them over the top-left corner, and a field
 * starting at the very top sits under them.
 */
export function Sidebar(props: {
  colors: ApplicationTheme;
  groups: NavGroup[];
  current: string;
  search: string;
  /** Points the window's own controls need above the first row. */
  topInset: number;
  onSearch: (value: string) => void;
  onNavigate: (route: string) => void;
}) {
  const query = props.search.trim().toLowerCase();
  return (
    <View
      class="h-full shrink-0 flex-col"
      style={{
        width: M.sidebarWidth,
        backgroundColor: props.colors.shell,
        paddingTop: props.topInset,
        paddingBottom: M.sidebarTop,
        paddingLeft: GAP.padX,
        paddingRight: GAP.padX,
      }}
    >
      <View class="min-h-0 flex-1 flex-col" style={{ gap: GAP.searchGap }}>
        <SearchField
          value={props.search}
          placeholder="Поиск настроек"
          colors={props.colors}
          onChange={props.onSearch}
        />
        <Scroll class="min-h-0 flex-1">
          <View style={{ gap: GAP.groupGap }}>
            {props.groups.map((group, index) => {
              const entries = group.entries.filter(
                (entry) => !query || entry.label.toLowerCase().includes(query),
              );
              if (!entries.length) return null;
              return (
                <View
                  key={group.id}
                  style={{
                    gap: GAP.entryGap,
                    marginBottom: index === 0 ? 0 : 0,
                  }}
                >
                  {entries.map((entry) => (
                    <SidebarRow
                      key={entry.route}
                      label={entry.label}
                      icon={entry.icon}
                      caption={entry.caption}
                      logo={entry.route === "server"}
                      selected={props.current === entry.route}
                      colors={props.colors}
                      onPress={() => props.onNavigate(entry.route)}
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
