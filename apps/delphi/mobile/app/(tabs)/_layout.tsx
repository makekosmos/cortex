import React from "react";
import { Pressable } from "react-native";
import { Tabs, useRouter } from "expo-router";
import { Ionicons } from "@expo/vector-icons";
import useTodoStore from "@/store/todos";
import { SmartList } from "@/types/task";
import { countAll } from "@/services/filters/todoFilterService";

export default function TabLayout() {
  const todos = useTodoStore((s) => s.todos);
  const counts = countAll(todos);
  const router = useRouter();

  const settingsButton = () => (
    <Pressable onPress={() => router.push("/settings")} hitSlop={8} style={{ marginRight: 16 }}>
      <Ionicons name="settings-outline" size={22} color="rgba(255,255,255,0.6)" />
    </Pressable>
  );

  return (
    <Tabs
      screenOptions={{
        headerRight: settingsButton,
        tabBarStyle: {
          backgroundColor: "#111",
          borderTopColor: "rgba(255,255,255,0.08)",
        },
        tabBarActiveTintColor: "#60a5fa",
        tabBarInactiveTintColor: "rgba(255,255,255,0.4)",
        headerStyle: { backgroundColor: "#0a0a0a" },
        headerTintColor: "#f5f5f5",
        headerShadowVisible: false,
      }}
    >
      <Tabs.Screen
        name="index"
        options={{
          title: "Сегодня",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="star" size={size} color={color} />
          ),
          tabBarBadge:
            counts[SmartList.Today] > 0
              ? counts[SmartList.Today]
              : undefined,
          headerTitle: "Сегодня",
        }}
      />
      <Tabs.Screen
        name="inbox"
        options={{
          title: "Входящие",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="mail-outline" size={size} color={color} />
          ),
          tabBarBadge:
            counts[SmartList.Inbox] > 0
              ? counts[SmartList.Inbox]
              : undefined,
          headerTitle: "Входящие",
        }}
      />
      <Tabs.Screen
        name="upcoming"
        options={{
          title: "Планы",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="calendar" size={size} color={color} />
          ),
          tabBarBadge:
            counts[SmartList.Upcoming] > 0
              ? counts[SmartList.Upcoming]
              : undefined,
          headerTitle: "Планы",
        }}
      />
      <Tabs.Screen
        name="logbook"
        options={{
          title: "Журнал",
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="book" size={size} color={color} />
          ),
          headerTitle: "Журнал",
        }}
      />
    </Tabs>
  );
}
