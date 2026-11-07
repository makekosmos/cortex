import { Text, View } from "nsh";
import {
  Card,
  CardRow,
  Icon,
  SectionLabel,
  SettingsPage,
  Switch,
  ThemeCard,
  darkThemes,
  lightThemes,
} from "@makekosmos/imago-nsh";
import type { PageProps } from "../commands";
import type { ApplicationTheme, PaletteTheme } from "@makekosmos/imago-nsh";

function ThemeRow(props: {
  title: string;
  themes: PaletteTheme[];
  selected: string;
  colors: ApplicationTheme;
  onPick: (key: string) => void;
}) {
  return (
    <View>
      <SectionLabel colors={props.colors}>{props.title}</SectionLabel>
      <View class="flex-row" style={{ gap: 14 }}>
        {props.themes.map((theme) => (
          <ThemeCard
            key={theme.key}
            name={theme.name}
            colors={theme.colors}
            selected={props.selected === theme.key}
            active={props.selected === theme.key}
            captionColor={
              props.selected === theme.key ? props.colors.foreground : props.colors.muted
            }
            onPress={() => props.onPick(theme.key)}
          />
        ))}
      </View>
    </View>
  );
}

/**
 * Themes and palette. The choice is device-wide and lives in the Engine
 * (`appearance.get`/`appearance.set`), so every Mundus window on this computer
 * wears the same one — the pages only read it and ask for a change.
 */
export function AppearancePage(props: PageProps) {
  const state = () => props.state();
  const follow = () => state().appearance.followApps;
  return (
    <SettingsPage colors={props.colors} title="Внешний вид">
      <ThemeRow
        title={`Светлая тема${state().appearance.mode === "light" ? " — активна" : ""}`}
        themes={lightThemes}
        selected={state().appearance.light}
        colors={props.colors}
        onPick={(key) => props.onCommand("appearance.theme", { mode: "light", key })}
      />

      <ThemeRow
        title={`Тёмная тема${state().appearance.mode === "dark" ? " — активна" : ""}`}
        themes={darkThemes}
        selected={state().appearance.dark}
        colors={props.colors}
        onPick={(key) => props.onCommand("appearance.theme", { mode: "dark", key })}
      />

      <View>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            title="Следовать теме приложений"
            subtitle={
              follow()
                ? "Mundus берёт светлую или тёмную тему из системы"
                : "Тема выбирается здесь, независимо от системы"
            }
            last
            right={
              <Switch
                colors={props.colors}
                value={follow()}
                accessibilityLabel="Следовать теме приложений"
                onChange={(value) => props.onCommand("appearance.followApps", { value })}
              />
            }
          />
        </Card>
      </View>

      <View>
        <SectionLabel colors={props.colors}>Интерфейс</SectionLabel>
        <Card colors={props.colors}>
          <CardRow
            colors={props.colors}
            title="Акцентный цвет"
            subtitle={
              state().appearance.accentSource === "wallpaper" ? "Взят с обоев" : "Взят из темы"
            }
            right={
              <View
                style={{
                  width: 26,
                  height: 26,
                  borderRadius: 13,
                  backgroundColor: props.colors.accent,
                }}
              />
            }
            last
          />
        </Card>
      </View>

      <View class="flex-row items-center gap-1.5">
        <Icon name="palette" size={13} color={props.colors.faint} />
        <Text style={{ fontSize: 12, color: props.colors.faint }}>
          Тема применяется ко всем окнам Mundus на этом компьютере.
        </Text>
      </View>
    </SettingsPage>
  );
}
