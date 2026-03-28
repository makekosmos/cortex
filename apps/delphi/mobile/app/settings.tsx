import React, { useState, useEffect, useCallback } from "react";
import {
  View,
  Text,
  TextInput,
  Pressable,
  StyleSheet,
  Alert,
  ScrollView,
} from "react-native";
import { Ionicons } from "@expo/vector-icons";
import { parseConnectionString } from "@/services/sync/pairing";
import { arkSync, fetchTasksFromArk } from "@/services/sync/ark-client";
import { getSetting, setSetting, deleteSetting } from "@/db/storage";
import useTodoStore from "@/store/todos";

export default function SettingsScreen() {
  const [connectionString, setConnectionString] = useState("");
  const [arkUrl, setArkUrl] = useState("");
  const [connected, setConnected] = useState(arkSync.isConnected);
  const setTodos = useTodoStore((s) => s.setTodos);

  useEffect(() => {
    (async () => {
      const url = await getSetting("ark_url");
      if (url) setArkUrl(url);
    })();

    const unsub = arkSync.onStatus((status) => setConnected(status));
    return unsub;
  }, []);

  const handlePair = useCallback(async () => {
    const conn = parseConnectionString(connectionString.trim());
    if (!conn) {
      Alert.alert("Ошибка", "Неверный формат строки подключения.\nФормат: ark://host:port?key=SECRET");
      return;
    }

    await setSetting("ark_url", conn.server_url);
    await setSetting("ark_api_key", conn.api_key);
    setArkUrl(conn.server_url);

    // Fetch initial data
    const todos = await fetchTasksFromArk(conn.server_url, conn.api_key);
    if (todos.length > 0) {
      setTodos(todos);
    }

    // Connect WebSocket
    arkSync.disconnect();
    arkSync.connect(conn.server_url, conn.api_key);
    setConnectionString("");
  }, [connectionString, setTodos]);

  const handleDisconnect = useCallback(async () => {
    arkSync.disconnect();
    await deleteSetting("ark_url");
    await deleteSetting("ark_api_key");
    setArkUrl("");
    setConnected(false);
  }, []);

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.sectionTitle}>Ark Sync</Text>

      {arkUrl ? (
        <View style={styles.card}>
          <View style={styles.statusRow}>
            <View
              style={[
                styles.statusDot,
                { backgroundColor: connected ? "#4ade80" : "#f87171" },
              ]}
            />
            <Text style={styles.statusText}>
              {connected ? "Подключено" : "Отключено"}
            </Text>
          </View>
          <Text style={styles.urlText}>{arkUrl}</Text>
          <Pressable style={styles.disconnectButton} onPress={handleDisconnect}>
            <Text style={styles.disconnectText}>Отключиться</Text>
          </Pressable>
        </View>
      ) : (
        <View style={styles.card}>
          <Text style={styles.description}>
            Введите строку подключения для синхронизации с Ark сервером.
          </Text>
          <TextInput
            style={styles.input}
            value={connectionString}
            onChangeText={setConnectionString}
            placeholder="ark://192.168.1.5:8000?key=..."
            placeholderTextColor="rgba(255,255,255,0.3)"
            autoCapitalize="none"
            autoCorrect={false}
          />
          <Pressable style={styles.pairButton} onPress={handlePair}>
            <Ionicons name="link" size={18} color="#fff" />
            <Text style={styles.pairText}>Подключить</Text>
          </Pressable>
        </View>
      )}

      <Text style={styles.sectionTitle}>О приложении</Text>
      <View style={styles.card}>
        <Text style={styles.aboutText}>
          Delphi Mobile v1.0.0
        </Text>
        <Text style={styles.aboutSubtext}>
          GTD-менеджер задач. Часть экосистемы Kosmos.
        </Text>
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
  },
  content: {
    padding: 16,
  },
  sectionTitle: {
    color: "rgba(255,255,255,0.5)",
    fontSize: 13,
    fontWeight: "600",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    marginBottom: 8,
    marginTop: 24,
    marginLeft: 4,
  },
  card: {
    backgroundColor: "rgba(255,255,255,0.06)",
    borderRadius: 12,
    padding: 16,
  },
  description: {
    color: "rgba(255,255,255,0.6)",
    fontSize: 14,
    lineHeight: 20,
    marginBottom: 12,
  },
  input: {
    color: "#f5f5f5",
    fontSize: 15,
    paddingVertical: 10,
    paddingHorizontal: 12,
    backgroundColor: "rgba(255,255,255,0.06)",
    borderRadius: 8,
    borderWidth: 1,
    borderColor: "rgba(255,255,255,0.1)",
    marginBottom: 12,
  },
  pairButton: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: 6,
    backgroundColor: "#3b82f6",
    borderRadius: 8,
    paddingVertical: 10,
  },
  pairText: {
    color: "#fff",
    fontSize: 15,
    fontWeight: "600",
  },
  statusRow: {
    flexDirection: "row",
    alignItems: "center",
    marginBottom: 8,
  },
  statusDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    marginRight: 8,
  },
  statusText: {
    color: "#f5f5f5",
    fontSize: 15,
    fontWeight: "500",
  },
  urlText: {
    color: "rgba(255,255,255,0.5)",
    fontSize: 13,
    marginBottom: 12,
  },
  disconnectButton: {
    backgroundColor: "rgba(248,113,113,0.15)",
    borderRadius: 8,
    paddingVertical: 10,
    alignItems: "center",
  },
  disconnectText: {
    color: "#f87171",
    fontSize: 15,
    fontWeight: "600",
  },
  aboutText: {
    color: "#f5f5f5",
    fontSize: 15,
    fontWeight: "500",
  },
  aboutSubtext: {
    color: "rgba(255,255,255,0.4)",
    fontSize: 13,
    marginTop: 4,
  },
});
