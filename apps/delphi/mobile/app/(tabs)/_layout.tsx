import React from "react";
import { Text, Pressable } from "react-native";
import { Tabs, useRouter } from "expo-router";
import useTodoStore from "@/store/todos";
import { SmartList } from "@/types/task";
import { countAll } from "@/services/filters/todoFilterService";

function TabIcon({ label, color }: { label: string; color: string }) {
  return <Text style={{ fontSize: 20, color }}>{label}</Text>;
}

export default function TabLayout() {
  const todos = useTodoStore((s) => s.todos) ?? [];
  const counts = countAll(todos);
  const router = useRouter();

  return (
    <Tabs
      screenOptions={{
        headerRight: () => (
          <Pressable
            onPress={() => router.push("/settings")}
            hitSlop={8}
            style={{ marginRight: 16 }}
          >
            <Text style={{ fontSize: 20 }}>⚙️</Text>
          </Pressable>
        ),
        tabBarStyle: {
          backgroundColor: "#111",
          borderTopColor: "rgba(255,255,255,0.08)",
          paddingBottom: 8,
          height: 60,
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
          tabBarIcon: ({ color }) => <TabIcon label="⭐" color={color} />,
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
          tabBarIcon: ({ color }) => <TabIcon label="📥" color={color} />,
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
          tabBarIcon: ({ color }) => <TabIcon label="📅" color={color} />,
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
          tabBarIcon: ({ color }) => <TabIcon label="📖" color={color} />,
          headerTitle: "Журнал",
        }}
      />
    </Tabs>
  );
}
