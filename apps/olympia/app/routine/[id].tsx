import { useState, useEffect } from "react";

import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  TouchableOpacity,
  Alert,
} from "react-native";

import { useLocalSearchParams, router, Stack } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { useThemeColor } from "@/lib/useThemeColor";

import { getRoutineWithExercises, deleteRoutine } from "@/lib/database";

export default function RoutineDetailScreen() {
  const colors = useThemeColor();

  const { id } = useLocalSearchParams<{ id: string }>();

  const [routine, setRoutine] = useState<any>(null);

  useEffect(() => {
    if (id) getRoutineWithExercises(id).then(setRoutine);
  }, [id]);

  function handleDelete() {
    Alert.alert("Удалить программу?", "Это действие нельзя отменить.", [
      { text: "Отмена", style: "cancel" },

      {
        text: "Удалить",

        style: "destructive",

        onPress: async () => {
          await deleteRoutine(id);
          router.back();
        },
      },
    ]);
  }

  if (!routine)
    return (
      <View
        style={[styles.container, { backgroundColor: colors.background }]}
      />
    );

  return (
    <>
      <Stack.Screen
        options={{
          title: routine.title,

          headerRight: () => (
            <TouchableOpacity onPress={handleDelete}>
              <Ionicons name="trash-outline" size={20} color={colors.danger} />
            </TouchableOpacity>
          ),
        }}
      />
      <ScrollView
        style={[styles.container, { backgroundColor: colors.background }]}
      >
        {routine.exercises?.map((ex: any, i: number) => (
          <View
            key={ex.id}
            style={[styles.exerciseRow, { backgroundColor: colors.surface }]}
          >
            <Text style={[styles.index, { color: colors.textTertiary }]}>
              {i + 1}
            </Text>
            <View style={{ flex: 1 }}>
              <Text style={[styles.exerciseName, { color: colors.text }]}>
                {ex.exercise_name}
              </Text>
              <Text style={[styles.target, { color: colors.textSecondary }]}>
                {ex.target_sets || 3} подх. × {ex.target_reps || "8-12"} повт.
              </Text>
            </View>
          </View>
        ))}
        <View style={{ height: 40 }} />
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },

  exerciseRow: {
    flexDirection: "row",
    alignItems: "center",
    marginHorizontal: 16,

    marginTop: 8,
    padding: 14,
    borderRadius: 10,
    gap: 12,
  },

  index: { fontSize: 16, fontWeight: "700", width: 24, textAlign: "center" },

  exerciseName: { fontSize: 15, fontWeight: "600" },

  target: { fontSize: 13, marginTop: 2 },
});
