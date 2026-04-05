import React from "react";

import { Tabs } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { useSafeAreaInsets } from "react-native-safe-area-context";

import Colors from "@/constants/Colors";

import { useSettingsStore } from "@/lib/stores/settings-store";

export default function TabLayout() {
  const theme = useSettingsStore((s) => s.theme);

  const colors = Colors[theme];

  const insets = useSafeAreaInsets();

  return (
    <Tabs
      screenOptions={{
        tabBarActiveTintColor: colors.accent,

        tabBarInactiveTintColor: colors.tabIconDefault,

        tabBarStyle: {
          backgroundColor: colors.surface,

          borderTopColor: colors.border,

          height: 56 + insets.bottom,

          paddingBottom: insets.bottom,
        },

        headerStyle: { backgroundColor: colors.background },

        headerTintColor: colors.text,

        headerShadowVisible: false,

        sceneStyle: { backgroundColor: colors.background },

        lazy: false,

        animation: "shift",
      }}
    >
      <Tabs.Screen
        name="index"
        options={{
          title: "Программы",

          tabBarIcon: ({ color, size }) => (
            <Ionicons name="barbell-outline" size={size} color={color} />
          ),
        }}
      />
      <Tabs.Screen
        name="profile"
        options={{
          title: "Профиль",

          tabBarIcon: ({ color, size }) => (
            <Ionicons name="person-outline" size={size} color={color} />
          ),
        }}
      />
    </Tabs>
  );
}
