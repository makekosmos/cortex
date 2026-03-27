import { useState, useEffect } from 'react';
import {
  View, Text, StyleSheet, ScrollView, Alert, TouchableOpacity,
} from 'react-native';
import { useLocalSearchParams, router, Stack } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useThemeColor } from '@/lib/useThemeColor';
import { useSettingsStore } from '@/lib/stores/settings-store';
import { getWorkoutWithDetails, deleteWorkout } from '@/lib/database';

export default function WorkoutDetailScreen() {
  const colors = useThemeColor();
  const units = useSettingsStore((s) => s.units);
  const { id } = useLocalSearchParams<{ id: string }>();
  const [workout, setWorkout] = useState<any>(null);

  useEffect(() => {
    if (id) getWorkoutWithDetails(id).then(setWorkout);
  }, [id]);

  function formatDuration(seconds: number | null) {
    if (!seconds) return '--';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    if (h > 0) return `${h}ч ${m}м`;
    return `${m}м`;
  }

  function displayWeight(kg: number | null) {
    if (kg === null || kg === undefined) return '-';
    if (units === 'lbs') return `${(kg * 2.205).toFixed(0)} lbs`;
    return `${kg} кг`;
  }

  function handleDelete() {
    Alert.alert('Удалить тренировку?', 'Это действие нельзя отменить.', [
      { text: 'Отмена', style: 'cancel' },
      {
        text: 'Удалить',
        style: 'destructive',
        onPress: async () => { await deleteWorkout(id); router.back(); },
      },
    ]);
  }

  if (!workout) return <View style={[styles.container, { backgroundColor: colors.background }]} />;

  return (
    <>
      <Stack.Screen
        options={{
          title: workout.title,
          headerRight: () => (
            <TouchableOpacity onPress={handleDelete}>
              <Ionicons name="trash-outline" size={20} color={colors.danger} />
            </TouchableOpacity>
          ),
        }}
      />
      <ScrollView style={[styles.container, { backgroundColor: colors.background }]}>
        <View style={styles.meta}>
          <Text style={[styles.date, { color: colors.textSecondary }]}>
            {new Date(workout.started_at).toLocaleDateString('ru-RU', {
              weekday: 'long', year: 'numeric', month: 'long', day: 'numeric',
            })}
          </Text>
          <Text style={[styles.duration, { color: colors.textSecondary }]}>
            Длительность: {formatDuration(workout.duration_seconds)}
          </Text>
        </View>

        {workout.exercises?.map((ex: any) => (
          <View key={ex.id} style={[styles.exerciseBlock, { backgroundColor: colors.surface }]}>
            <Text style={[styles.exerciseName, { color: colors.accent }]}>{ex.exercise_name}</Text>
            {ex.sets?.map((set: any, i: number) => (
              <View key={set.id} style={styles.setRow}>
                <Text style={[styles.setNum, { color: colors.textSecondary }]}>{i + 1}</Text>
                <Text style={[styles.setText, { color: colors.text }]}>
                  {displayWeight(set.weight_kg)} × {set.reps ?? '-'} повт.
                </Text>
                {set.rpe ? (
                  <Text style={[styles.rpe, { color: colors.textTertiary }]}>RPE {set.rpe}</Text>
                ) : null}
              </View>
            ))}
          </View>
        ))}
        <View style={{ height: 40 }} />
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  meta: { padding: 16, gap: 4 },
  date: { fontSize: 15 },
  duration: { fontSize: 14 },
  exerciseBlock: { marginHorizontal: 16, marginBottom: 10, padding: 14, borderRadius: 10 },
  exerciseName: { fontSize: 15, fontWeight: '700', marginBottom: 8 },
  setRow: { flexDirection: 'row', alignItems: 'center', paddingVertical: 4, gap: 12 },
  setNum: { width: 24, fontSize: 14, fontWeight: '600' },
  setText: { fontSize: 14, flex: 1 },
  rpe: { fontSize: 13 },
});
