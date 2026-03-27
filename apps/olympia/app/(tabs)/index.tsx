import { useState, useCallback, useMemo, useEffect } from 'react';
import {
  View, Text, StyleSheet, TouchableOpacity, ScrollView, Alert,
  TextInput, FlatList, ActivityIndicator,
} from 'react-native';
import { router, useFocusEffect } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useThemeColor } from '@/lib/useThemeColor';
import { useWorkoutStore } from '@/lib/stores/workout-store';
import { useExercisesStore } from '@/lib/stores/exercises-store';
import { useSettingsStore } from '@/lib/stores/settings-store';
import {
  getRoutines, getRoutineWithExercises, getExerciseById,
} from '@/lib/database';
import { MUSCLE_GROUPS, musclesRu, muscleRu } from '@/lib/types';
import React from 'react';

const PAGE_SIZE = 30;

const ExerciseRow = React.memo(({ item, colors }: { item: any; colors: any }) => (
  <TouchableOpacity
    style={[styles.exerciseRow, { borderBottomColor: colors.border }]}
    onPress={() => router.push(`/exercise/${item.id}`)}
    activeOpacity={0.6}
  >
    <View style={{ flex: 1 }}>
      <Text style={[styles.exerciseName, { color: colors.text }]}>{item.name}</Text>
      <Text style={[styles.exerciseMeta, { color: colors.textSecondary }]}>
        {musclesRu(item.primary_muscles)} {item.equipment ? `· ${item.equipment}` : ''}
      </Text>
    </View>
    <Ionicons name="chevron-forward" size={18} color={colors.textTertiary} />
  </TouchableOpacity>
));

export default function ProgramsTab() {
  const colors = useThemeColor();
  const isActive = useWorkoutStore((s) => s.isActive);
  const startWorkout = useWorkoutStore((s) => s.startWorkout);
  const startFromRoutine = useWorkoutStore((s) => s.startFromRoutine);
  const exercises = useExercisesStore((s) => s.exercises);
  const [routines, setRoutines] = useState<any[]>([]);
  const [tab, setTab] = useState<'programs' | 'exercises'>('programs');

  // Exercise catalog state
  const [search, setSearch] = useState('');
  const [selectedMuscle, setSelectedMuscle] = useState<string | null>(null);
  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  useFocusEffect(
    useCallback(() => {
      getRoutines().then(setRoutines);
    }, [])
  );

  useEffect(() => { setVisibleCount(PAGE_SIZE); }, [search, selectedMuscle]);

  const filtered = useMemo(() => {
    let result = exercises;
    if (search) {
      const q = search.toLowerCase();
      result = result.filter((e) => e.name.toLowerCase().includes(q));
    }
    if (selectedMuscle) {
      result = result.filter((e) => e.primary_muscles.includes(selectedMuscle));
    }
    return result;
  }, [exercises, search, selectedMuscle]);

  const visible = useMemo(() => filtered.slice(0, visibleCount), [filtered, visibleCount]);
  const hasMore = visibleCount < filtered.length;

  function handleStartEmpty() {
    if (isActive) {
      Alert.alert('Тренировка активна', 'Отменить текущую и начать новую?', [
        { text: 'Нет', style: 'cancel' },
        {
          text: 'Да',
          style: 'destructive',
          onPress: () => {
            useWorkoutStore.getState().cancelWorkout();
            startWorkout();
            router.push('/workout/live');
          },
        },
      ]);
      return;
    }
    startWorkout();
    router.push('/workout/live');
  }

  async function handleStartFromRoutine(routineId: string) {
    const routine = await getRoutineWithExercises(routineId);
    if (!routine) return;
    const exercisesWithData = await Promise.all(
      routine.exercises.map(async (re: any) => ({
        exercise: await getExerciseById(re.exercise_id),
        targetSets: re.target_sets || 3,
        targetWeight: re.target_weight_kg,
      }))
    );

    if (isActive) {
      Alert.alert('Тренировка активна', 'Отменить текущую?', [
        { text: 'Нет', style: 'cancel' },
        {
          text: 'Да',
          style: 'destructive',
          onPress: async () => {
            useWorkoutStore.getState().cancelWorkout();
            await startFromRoutine(routine.title, exercisesWithData);
            router.push('/workout/live');
          },
        },
      ]);
      return;
    }
    await startFromRoutine(routine.title, exercisesWithData);
    router.push('/workout/live');
  }

  return (
    <View style={[styles.container, { backgroundColor: colors.background }]}>
      {/* Resume banner */}
      {isActive && (
        <TouchableOpacity
          style={[styles.resumeBanner, { backgroundColor: colors.success }]}
          onPress={() => router.push('/workout/live')}
          activeOpacity={0.7}
        >
          <Ionicons name="play-circle" size={20} color="#fff" />
          <Text style={styles.resumeText}>Продолжить тренировку</Text>
          <Ionicons name="chevron-forward" size={20} color="#fff" />
        </TouchableOpacity>
      )}

      {/* Tab switcher */}
      <View style={[styles.tabSwitcher, { backgroundColor: colors.surface }]}>
        <TouchableOpacity
          style={[styles.tabBtn, tab === 'programs' && { backgroundColor: colors.accent }]}
          onPress={() => setTab('programs')}
        >
          <Text style={[styles.tabBtnText, { color: tab === 'programs' ? '#fff' : colors.textSecondary }]}>
            Программы
          </Text>
        </TouchableOpacity>
        <TouchableOpacity
          style={[styles.tabBtn, tab === 'exercises' && { backgroundColor: colors.accent }]}
          onPress={() => setTab('exercises')}
        >
          <Text style={[styles.tabBtnText, { color: tab === 'exercises' ? '#fff' : colors.textSecondary }]}>
            Упражнения
          </Text>
        </TouchableOpacity>
      </View>

      {tab === 'programs' ? (
        <ScrollView contentContainerStyle={styles.programsContent}>
          {/* Start empty */}
          <TouchableOpacity
            style={[styles.startBtn, { backgroundColor: colors.accent }]}
            onPress={handleStartEmpty}
            activeOpacity={0.7}
          >
            <Ionicons name="flash" size={20} color="#fff" />
            <Text style={styles.startBtnText}>Свободная тренировка</Text>
          </TouchableOpacity>

          {/* Create routine */}
          <TouchableOpacity
            style={[styles.createBtn, { backgroundColor: colors.surface }]}
            onPress={() => router.push('/routine/create')}
            activeOpacity={0.7}
          >
            <Ionicons name="add-circle-outline" size={20} color={colors.accent} />
            <Text style={[styles.createBtnText, { color: colors.accent }]}>Создать программу</Text>
          </TouchableOpacity>

          {/* Routines */}
          {routines.length === 0 ? (
            <View style={styles.emptyState}>
              <Ionicons name="clipboard-outline" size={48} color={colors.textTertiary} />
              <Text style={[styles.emptyTitle, { color: colors.textSecondary }]}>
                Пока нет программ
              </Text>
              <Text style={[styles.emptySubtitle, { color: colors.textTertiary }]}>
                Создайте программу из набора упражнений
              </Text>
            </View>
          ) : (
            <>
              <Text style={[styles.sectionTitle, { color: colors.text }]}>Мои программы</Text>
              {routines.map((item) => (
                <TouchableOpacity
                  key={item.id}
                  style={[styles.routineCard, { backgroundColor: colors.surface }]}
                  onPress={() => handleStartFromRoutine(item.id)}
                  onLongPress={() => router.push(`/routine/${item.id}`)}
                  activeOpacity={0.7}
                >
                  <View style={{ flex: 1 }}>
                    <Text style={[styles.routineTitle, { color: colors.text }]}>{item.title}</Text>
                    <Text style={[styles.routineHint, { color: colors.textTertiary }]}>
                      Нажмите для старта · Удерживайте для деталей
                    </Text>
                  </View>
                  <Ionicons name="play" size={22} color={colors.accent} />
                </TouchableOpacity>
              ))}
            </>
          )}

          <View style={{ height: 20 }} />
        </ScrollView>
      ) : (
        <View style={{ flex: 1 }}>
          {/* Search */}
          <View style={[styles.searchContainer, { backgroundColor: colors.surface }]}>
            <Ionicons name="search" size={18} color={colors.textTertiary} />
            <TextInput
              style={[styles.searchInput, { color: colors.text }]}
              placeholder="Поиск упражнений..."
              placeholderTextColor={colors.textTertiary}
              value={search}
              onChangeText={setSearch}
              autoCorrect={false}
            />
            {search.length > 0 && (
              <TouchableOpacity onPress={() => setSearch('')}>
                <Ionicons name="close-circle" size={18} color={colors.textTertiary} />
              </TouchableOpacity>
            )}
          </View>

          {/* Muscle filters */}
          <ScrollView horizontal showsHorizontalScrollIndicator={false} style={styles.filterRow}>
            <TouchableOpacity
              style={[styles.chip, { backgroundColor: !selectedMuscle ? colors.accent : colors.surface }]}
              onPress={() => setSelectedMuscle(null)}
            >
              <Text style={[styles.chipText, { color: !selectedMuscle ? '#fff' : colors.textSecondary }]}>Все</Text>
            </TouchableOpacity>
            {MUSCLE_GROUPS.map((m) => (
              <TouchableOpacity
                key={m}
                style={[styles.chip, { backgroundColor: selectedMuscle === m ? colors.accent : colors.surface }]}
                onPress={() => setSelectedMuscle(selectedMuscle === m ? null : m)}
              >
                <Text style={[styles.chipText, { color: selectedMuscle === m ? '#fff' : colors.textSecondary }]}>
                  {muscleRu(m)}
                </Text>
              </TouchableOpacity>
            ))}
          </ScrollView>

          <View style={styles.exercisesHeader}>
            <Text style={[styles.count, { color: colors.textTertiary }]}>
              {filtered.length} упражнений
            </Text>
            <TouchableOpacity
              style={[styles.createExBtn, { backgroundColor: colors.surface }]}
              onPress={() => router.push('/exercise/create')}
              activeOpacity={0.7}
            >
              <Ionicons name="add" size={16} color={colors.accent} />
              <Text style={[styles.createExText, { color: colors.accent }]}>Создать своё</Text>
            </TouchableOpacity>
          </View>

          <FlatList
            data={visible}
            keyExtractor={(item) => item.id}
            contentContainerStyle={{ paddingHorizontal: 16, paddingBottom: 20 }}
            renderItem={({ item }) => <ExerciseRow item={item} colors={colors} />}
            onEndReached={() => hasMore && setVisibleCount((p) => p + PAGE_SIZE)}
            onEndReachedThreshold={0.5}
            initialNumToRender={PAGE_SIZE}
            maxToRenderPerBatch={15}
            windowSize={5}
            removeClippedSubviews={true}
            getItemLayout={(_, index) => ({ length: 60, offset: 60 * index, index })}
            ListFooterComponent={
              hasMore ? <ActivityIndicator style={{ paddingVertical: 16 }} color={colors.accent} /> : null
            }
          />
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  resumeBanner: {
    flexDirection: 'row', alignItems: 'center', margin: 16, marginBottom: 0,
    padding: 14, borderRadius: 12, gap: 8,
  },
  resumeText: { color: '#fff', fontSize: 15, fontWeight: '600', flex: 1 },
  tabSwitcher: {
    flexDirection: 'row', margin: 16, borderRadius: 10, padding: 3,
  },
  tabBtn: { flex: 1, paddingVertical: 8, borderRadius: 8, alignItems: 'center' },
  tabBtnText: { fontSize: 14, fontWeight: '600' },
  programsContent: { paddingHorizontal: 16 },
  startBtn: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    padding: 16, borderRadius: 12, gap: 8, marginBottom: 10,
  },
  startBtnText: { color: '#fff', fontSize: 16, fontWeight: '700' },
  createBtn: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    padding: 14, borderRadius: 10, gap: 6, marginBottom: 20,
  },
  createBtnText: { fontSize: 15, fontWeight: '600' },
  sectionTitle: { fontSize: 18, fontWeight: '700', marginBottom: 10 },
  emptyState: { alignItems: 'center', marginTop: 40, gap: 8, paddingHorizontal: 20 },
  emptyTitle: { fontSize: 16, fontWeight: '600' },
  emptySubtitle: { fontSize: 13, textAlign: 'center' },
  routineCard: {
    flexDirection: 'row', alignItems: 'center',
    padding: 16, borderRadius: 10, marginBottom: 8,
  },
  routineTitle: { fontSize: 16, fontWeight: '600' },
  routineHint: { fontSize: 11, marginTop: 2 },
  searchContainer: {
    flexDirection: 'row', alignItems: 'center', margin: 16, marginBottom: 8,
    paddingHorizontal: 12, borderRadius: 10, height: 44, gap: 8,
  },
  searchInput: { flex: 1, fontSize: 16 },
  filterRow: { paddingLeft: 16, marginBottom: 8, maxHeight: 40 },
  chip: { paddingHorizontal: 14, paddingVertical: 6, borderRadius: 16, marginRight: 8 },
  chipText: { fontSize: 13, fontWeight: '500' },
  exercisesHeader: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: 16, marginBottom: 8,
  },
  createExBtn: {
    flexDirection: 'row', alignItems: 'center', paddingHorizontal: 10,
    paddingVertical: 5, borderRadius: 14, gap: 4,
  },
  createExText: { fontSize: 13, fontWeight: '600' },
  count: { fontSize: 13 },
  exerciseRow: {
    flexDirection: 'row', alignItems: 'center', paddingVertical: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  exerciseName: { fontSize: 15, fontWeight: '500' },
  exerciseMeta: { fontSize: 13, marginTop: 2 },
});
