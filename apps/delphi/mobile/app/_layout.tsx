import React, { useEffect, useCallback, useRef } from "react";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import useTodoStore from "@/store/todos";
import {
  arkSync,
  arkChangeToTodoItem,
  arkChangeToProject,
  arkChangeEventType,
} from "@/services/sync/ark-client";
import { getSetting } from "@/db/storage";

export default function RootLayout() {
  const hydrate = useTodoStore((s) => s.hydrate);
  const hydrated = useTodoStore((s) => s.hydrated);
  const upsertTodo = useTodoStore((s) => s.upsertTodo);
  const upsertProject = useTodoStore((s) => s.upsertProject);
  const initialized = useRef(false);

  // Hydrate store from SQLite on launch
  useEffect(() => {
    hydrate();
  }, [hydrate]);

  // Connect to Ark after hydration
  useEffect(() => {
    if (!hydrated || initialized.current) return;
    initialized.current = true;

    (async () => {
      const url = await getSetting("ark_url");
      const key = await getSetting("ark_api_key");
      if (url && key) {
        arkSync.connect(url, key);
      }
    })();

    const unsub = arkSync.onChange((change) => {
      const eventType = arkChangeEventType(change);
      if (eventType === "project") {
        const project = arkChangeToProject(change);
        if (project) upsertProject(project);
      } else {
        const todo = arkChangeToTodoItem(change);
        if (todo) upsertTodo(todo);
      }
    });

    return () => {
      unsub();
      arkSync.disconnect();
    };
  }, [hydrated, upsertTodo, upsertProject]);

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
