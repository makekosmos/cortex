import React, { useEffect, useRef, useState } from "react";
import { View, Text, ActivityIndicator } from "react-native";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import useTodoStore from "@/store/todos";
import {
  arkSync,
  arkChangeToTodoItem,
  arkChangeToProject,
  arkChangeEventType,
  fetchTasksFromArk,
} from "@/services/sync/ark-client";
import { getSetting } from "@/db/storage";

export default function RootLayout() {
  const hydrate = useTodoStore((s) => s.hydrate);
  const hydrated = useTodoStore((s) => s.hydrated);
  const upsertTodo = useTodoStore((s) => s.upsertTodo);
  const upsertProject = useTodoStore((s) => s.upsertProject);
  const upsertTodos = useTodoStore((s) => s.upsertTodos);
  const [error, setError] = useState<string | null>(null);

  // Hydrate store from SQLite on launch
  useEffect(() => {
    hydrate().catch((e: unknown) => {
      console.error("[RootLayout] hydrate failed:", e);
      setError(String(e));
    });
  }, [hydrate]);

  // Subscribe to Ark sync changes — always active
  useEffect(() => {
    const unsub = arkSync.onChange((change) => {
      try {
        const eventType = arkChangeEventType(change);
        if (eventType === "project") {
          const project = arkChangeToProject(change);
          if (project) upsertProject(project);
        } else {
          const todo = arkChangeToTodoItem(change);
          if (todo) upsertTodo(todo);
        }
      } catch (e) {
        console.warn("[RootLayout] Change handler error:", e);
      }
    });
    return unsub;
  }, [upsertTodo, upsertProject]);

  // Auto-connect to Ark if credentials exist
  useEffect(() => {
    if (!hydrated) return;

    (async () => {
      try {
        const url = await getSetting("ark_url");
        const key = await getSetting("ark_api_key");
        if (url && key && !url.includes("localhost") && !url.includes("127.0.0.1")) {
          if (!arkSync.isConnected) {
            arkSync.connect(url, key);
            // Fetch existing tasks via HTTP (outbox may not have everything)
            fetchTasksFromArk(url, key)
              .then((todos) => {
                if (todos.length > 0) {
                  console.log(`[RootLayout] Fetched ${todos.length} tasks from Ark HTTP`);
                  upsertTodos(todos);
                }
              })
              .catch((e) => console.warn("[RootLayout] HTTP fetch failed:", e));
          }
        } else if (url?.includes("localhost")) {
          const { deleteSetting } = await import("@/db/storage");
          await deleteSetting("ark_url");
          await deleteSetting("ark_api_key");
        }
      } catch (e) {
        console.warn("[RootLayout] Ark connect failed:", e);
      }
    })();

    return () => {
      arkSync.disconnect();
    };
  }, [hydrated]);

  if (error) {
    return (
      <View style={{ flex: 1, backgroundColor: "#0a0a0a", justifyContent: "center", alignItems: "center", padding: 20 }}>
        <Text style={{ color: "#f87171", fontSize: 16, marginBottom: 8 }}>Ошибка загрузки</Text>
        <Text style={{ color: "rgba(255,255,255,0.5)", fontSize: 13, textAlign: "center" }}>{error}</Text>
      </View>
    );
  }

  if (!hydrated) {
    return (
      <View style={{ flex: 1, backgroundColor: "#0a0a0a", justifyContent: "center", alignItems: "center" }}>
        <ActivityIndicator size="large" color="#60a5fa" />
      </View>
    );
  }

  return (
    <>
      <StatusBar style="light" />
      <Stack
        screenOptions={{
          headerShown: false,
          contentStyle: { backgroundColor: "#0a0a0a" },
        }}
      >
        <Stack.Screen name="(tabs)" />
        <Stack.Screen
          name="settings"
          options={{
            presentation: "modal",
            headerShown: true,
            headerTitle: "Настройки",
            headerStyle: { backgroundColor: "#111" },
            headerTintColor: "#f5f5f5",
          }}
        />
      </Stack>
    </>
  );
}
