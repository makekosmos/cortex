import { useEffect, useState } from "react";

import { Stack } from "expo-router";

import { StatusBar } from "expo-status-bar";

import { View, ActivityIndicator, StyleSheet } from "react-native";

import {
  DarkTheme,
  DefaultTheme,
  ThemeProvider,
} from "@react-navigation/native";

import { initializeDatabase, seedExercises } from "@/lib/database";

import { useSettingsStore } from "@/lib/stores/settings-store";

import { useExercisesStore } from "@/lib/stores/exercises-store";

import { useSyncStore } from "@/lib/sync/sync-store";

import Colors from "@/constants/Colors";

// Custom dark theme with our background color

const CustomDark = {
  ...DarkTheme,

  colors: {
    ...DarkTheme.colors,

    background: Colors.dark.background,

    card: Colors.dark.surface,

    border: Colors.dark.border,

    text: Colors.dark.text,

    primary: Colors.dark.accent,
  },
};

const CustomLight = {
  ...DefaultTheme,

  colors: {
    ...DefaultTheme.colors,

    background: Colors.light.background,

    card: Colors.light.surface,

    border: Colors.light.border,

    text: Colors.light.text,

    primary: Colors.light.accent,
  },
};

export default function RootLayout() {
  const [dbReady, setDbReady] = useState(false);

  const loadSettings = useSettingsStore((s) => s.loadSettings);

  const settingsLoaded = useSettingsStore((s) => s.loaded);

  const loadExercises = useExercisesStore((s) => s.load);

  const hydrateSync = useSyncStore((s) => s.hydrate);

  const theme = useSettingsStore((s) => s.theme);

  const colors = Colors[theme];

  const navTheme = theme === "dark" ? CustomDark : CustomLight;

  useEffect(() => {
    async function init() {
      await loadSettings();

      await initializeDatabase();

      await seedExercises();

      await loadExercises();

      await hydrateSync();

      setDbReady(true);
    }

    init();
  }, []);

  if (!dbReady || !settingsLoaded) {
    return (
      <View
        style={[styles.loading, { backgroundColor: Colors.dark.background }]}
      >
        <ActivityIndicator size="large" color={Colors.dark.accent} />
      </View>
    );
  }

  return (
    <ThemeProvider value={navTheme}>
      <StatusBar style={theme === "dark" ? "light" : "dark"} />
      <Stack
        screenOptions={{
          headerStyle: { backgroundColor: colors.background },

          headerTintColor: colors.text,

          contentStyle: { backgroundColor: colors.background },

          headerShadowVisible: false,

          animation: "fade",
        }}
      >
        <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
        <Stack.Screen
          name="workout/live"
          options={{
            presentation: "fullScreenModal",

            headerShown: false,

            animation: "slide_from_bottom",
          }}
        />
        <Stack.Screen
          name="workout/add-exercise"
          options={{
            presentation: "modal",

            title: "Добавить упражнение",

            animation: "slide_from_bottom",
          }}
        />
        <Stack.Screen
          name="exercise/create"
          options={{
            presentation: "modal",

            title: "Создать упражнение",

            animation: "slide_from_bottom",
          }}
        />
        <Stack.Screen name="exercise/[id]" options={{ title: "Упражнение" }} />
        <Stack.Screen
          name="workout/[id]"
          options={{ title: "Детали тренировки" }}
        />
        <Stack.Screen
          name="routine/create"
          options={{
            presentation: "modal",

            title: "Новая программа",

            animation: "slide_from_bottom",
          }}
        />
        <Stack.Screen name="routine/[id]" options={{ title: "Программа" }} />
        <Stack.Screen name="stats/strength" options={{ title: "" }} />
      </Stack>
    </ThemeProvider>
  );
}

const styles = StyleSheet.create({
  loading: { flex: 1, justifyContent: "center", alignItems: "center" },
});
