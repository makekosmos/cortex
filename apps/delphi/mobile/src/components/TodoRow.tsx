import React, { useCallback } from "react";
import {
  View,
  Text,
  Pressable,
  StyleSheet,
} from "react-native";
import type { TodoItem } from "@/types/task";
import { PriorityColor, Priority } from "@/types/task";
import useTodoStore from "@/store/todos";

type Props = {
  todo: TodoItem;
};

export default function TodoRow({ todo }: Props) {
  const completeTodo = useTodoStore((s) => s.completeTodo);
  const incompleteTodo = useTodoStore((s) => s.incompleteTodo);

  const onToggle = useCallback(() => {
    if (todo.isCompleted) {
      incompleteTodo(todo.id);
    } else {
      completeTodo(todo.id);
    }
  }, [todo.id, todo.isCompleted, completeTodo, incompleteTodo]);

  const priorityColor =
    todo.priority !== Priority.None
      ? PriorityColor[todo.priority]
      : undefined;

  return (
    <Pressable style={styles.container}>
      <Pressable onPress={onToggle} hitSlop={8} style={styles.checkbox}>
        <Text style={{ fontSize: 22, color: todo.isCompleted ? "#4ade80" : priorityColor ?? "rgba(255,255,255,0.4)" }}>
          {todo.isCompleted ? "✓" : "○"}
        </Text>
      </Pressable>
      <View style={styles.content}>
        <Text
          style={[styles.title, todo.isCompleted && styles.titleCompleted]}
          numberOfLines={1}
        >
          {todo.title}
        </Text>
        {todo.notes ? (
          <Text style={styles.notes} numberOfLines={1}>
            {todo.notes}
          </Text>
        ) : null}
        {(todo.scheduledDate || todo.deadline) ? (
          <View style={styles.meta}>
            {todo.scheduledDate ? (
              <Text style={styles.metaText}>
                {new Date(todo.scheduledDate).toLocaleDateString("ru-RU", {
                  day: "numeric",
                  month: "short",
                })}
              </Text>
            ) : null}
            {todo.deadline ? (
              <Text style={[styles.metaText, styles.deadline]}>
                {new Date(todo.deadline).toLocaleDateString("ru-RU", {
                  day: "numeric",
                  month: "short",
                })}
              </Text>
            ) : null}
          </View>
        ) : null}
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "flex-start",
    paddingVertical: 12,
    paddingHorizontal: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: "rgba(255,255,255,0.08)",
  },
  checkbox: {
    marginRight: 12,
    marginTop: 1,
  },
  content: {
    flex: 1,
  },
  title: {
    color: "#f5f5f5",
    fontSize: 16,
    lineHeight: 22,
  },
  titleCompleted: {
    color: "rgba(255,255,255,0.35)",
    textDecorationLine: "line-through",
  },
  notes: {
    color: "rgba(255,255,255,0.4)",
    fontSize: 13,
    lineHeight: 18,
    marginTop: 2,
  },
  meta: {
    flexDirection: "row",
    marginTop: 4,
    gap: 8,
  },
  metaText: {
    color: "rgba(255,255,255,0.5)",
    fontSize: 12,
  },
  deadline: {
    color: "#f87171",
  },
});
