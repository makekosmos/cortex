import React, { useCallback, useState } from "react";
import {
  View,
  FlatList,
  Text,
  TextInput,
  Pressable,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
} from "react-native";
import { Ionicons } from "@expo/vector-icons";
import { SmartList, SmartListTitle } from "@/types/task";
import useSmartList from "@/hooks/useSmartList";
import useTodoStore from "@/store/todos";
import TodoRow from "./TodoRow";
import type { TodoItem } from "@/types/task";

type Props = {
  list: SmartList;
};

export default function SmartListScreen({ list }: Props) {
  const { filtered } = useSmartList(list);
  const addTodo = useTodoStore((s) => s.addTodo);
  const [showInput, setShowInput] = useState(false);
  const [newTitle, setNewTitle] = useState("");

  const handleAdd = useCallback(() => {
    const title = newTitle.trim();
    if (!title) return;
    addTodo({
      title,
      isToday: list === SmartList.Today,
      isSomeday: list === SmartList.Someday,
    });
    setNewTitle("");
    setShowInput(false);
  }, [newTitle, addTodo, list]);

  const renderItem = useCallback(
    ({ item }: { item: TodoItem }) => <TodoRow todo={item} />,
    [],
  );

  const keyExtractor = useCallback((item: TodoItem) => item.id, []);

  const isReadonly = list === SmartList.Logbook || list === SmartList.Trash;

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === "ios" ? "padding" : "height"}
      keyboardVerticalOffset={100}
    >
      <FlatList
        data={filtered}
        renderItem={renderItem}
        keyExtractor={keyExtractor}
        contentContainerStyle={styles.list}
        ListEmptyComponent={
          <View style={styles.empty}>
            <Text style={styles.emptyText}>
              {list === SmartList.Logbook
                ? "Нет завершённых задач"
                : "Нет задач"}
            </Text>
          </View>
        }
      />

      {!isReadonly && (
        <>
          {showInput ? (
            <View style={styles.inputRow}>
              <TextInput
                style={styles.input}
                value={newTitle}
                onChangeText={setNewTitle}
                placeholder="Новая задача..."
                placeholderTextColor="rgba(255,255,255,0.3)"
                autoFocus
                returnKeyType="done"
                onSubmitEditing={handleAdd}
                onBlur={() => {
                  if (!newTitle.trim()) setShowInput(false);
                }}
              />
              <Pressable onPress={handleAdd} style={styles.sendButton}>
                <Ionicons name="arrow-up-circle" size={32} color="#60a5fa" />
              </Pressable>
            </View>
          ) : (
            <Pressable
              style={styles.fab}
              onPress={() => setShowInput(true)}
            >
              <Ionicons name="add" size={28} color="#fff" />
            </Pressable>
          )}
        </>
      )}
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: "#0a0a0a",
  },
  list: {
    flexGrow: 1,
  },
  empty: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    paddingTop: 120,
  },
  emptyText: {
    color: "rgba(255,255,255,0.3)",
    fontSize: 16,
  },
  inputRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: "rgba(255,255,255,0.1)",
    backgroundColor: "#111",
  },
  input: {
    flex: 1,
    color: "#f5f5f5",
    fontSize: 16,
    paddingVertical: 10,
    paddingHorizontal: 12,
    backgroundColor: "rgba(255,255,255,0.06)",
    borderRadius: 10,
  },
  sendButton: {
    marginLeft: 8,
  },
  fab: {
    position: "absolute",
    bottom: 20,
    right: 20,
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: "#3b82f6",
    justifyContent: "center",
    alignItems: "center",
    shadowColor: "#000",
    shadowOffset: { width: 0, height: 4 },
    shadowOpacity: 0.3,
    shadowRadius: 8,
    elevation: 8,
  },
});
