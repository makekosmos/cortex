import { useState, useEffect, useMemo, useRef } from 'react';
import {
  View, Text, StyleSheet, ScrollView, TouchableOpacity, TextInput, Alert, Vibration,
  Image, PanResponder, Animated,
} from 'react-native';
import { router } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useThemeColor } from '@/lib/useThemeColor';
import { useWorkoutStore } from '@/lib/stores/workout-store';
import { useSettingsStore } from '@/lib/stores/settings-store';
import * as Haptics from 'expo-haptics';
import { SafeAreaView, useSafeAreaInsets } from 'react-native-safe-area-context';

const SUPERSET_COLORS = ['#8B5CF6', '#F59E0B', '#EC4899', '#06B6D4', '#84CC16'];
const IMAGE_BASE = 'https://raw.githubusercontent.com/yuhonas/free-exercise-db/main/exercises/';
const SWIPE_VX = 0.3; // velocity threshold — much easier to trigger

export default function LiveWorkoutScreen() {
  const colors = useThemeColor();
  const insets = useSafeAreaInsets();
  const store = useWorkoutStore();
  const restTimerSeconds = useSettingsStore((s) => s.restTimerSeconds);
  const units = useSettingsStore((s) => s.units);
  const [elapsed, setElapsed] = useState(0);
  const [restRemaining, setRestRemaining] = useState(0);
  const [editingRestId, setEditingRestId] = useState<string | null>(null);
  const translateX = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    if (!store.startedAt) return;
    const interval = setInterval(() => {
      setElapsed(Math.floor((Date.now() - store.startedAt!.getTime()) / 1000));
    }, 1000);
    return () => clearInterval(interval);
  }, [store.startedAt]);

  useEffect(() => {
    if (!store.restTimerEnd) { setRestRemaining(0); return; }
    const tick = () => {
      const remaining = Math.max(0, Math.ceil((store.restTimerEnd!.getTime() - Date.now()) / 1000));
      setRestRemaining(remaining);
      if (remaining <= 0) {
        Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
        Vibration.vibrate([0, 500, 200, 500]);
        store.clearRestTimer();
      }
    };
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, [store.restTimerEnd]);

  const activeIndex = useMemo(() => {
    const idx = store.exercises.findIndex((e) => e.id === store.activeExerciseId);
    return idx >= 0 ? idx : 0;
  }, [store.exercises, store.activeExerciseId]);

  const activeExercise = store.exercises[activeIndex];
  const totalExercises = store.exercises.length;

  function goToExercise(idx: number) {
    if (idx >= 0 && idx < store.exercises.length) {
      store.setActiveExercise(store.exercises[idx].id);
      setEditingRestId(null);
    }
  }

  // Swipe handler — velocity-based, much lighter
  const panResponder = useRef(
    PanResponder.create({
      onMoveShouldSetPanResponder: (_, gs) =>
        Math.abs(gs.dx) > 10 && Math.abs(gs.dy) < 30,
      onPanResponderMove: (_, gs) => {
        translateX.setValue(gs.dx * 0.4); // dampened for visual feedback
      },
      onPanResponderRelease: (_, gs) => {
        const swiped = Math.abs(gs.vx) > SWIPE_VX || Math.abs(gs.dx) > 80;
        if (swiped && gs.dx < 0) {
          const idx = store.exercises.findIndex((e) => e.id === store.activeExerciseId);
          if (idx < store.exercises.length - 1) goToExercise(idx + 1);
        } else if (swiped && gs.dx > 0) {
          const idx = store.exercises.findIndex((e) => e.id === store.activeExerciseId);
          if (idx > 0) goToExercise(idx - 1);
        }
        Animated.spring(translateX, { toValue: 0, useNativeDriver: true, tension: 120, friction: 12 }).start();
      },
    })
  ).current;

  // "Сделал" — mark next uncompleted set, fill defaults, handle superset rotation
  function handleDone() {
    if (!activeExercise) return;

    const nextSet = activeExercise.sets.find((s) => !s.completed);
    if (!nextSet) {
      // All sets done on this exercise
      if (activeExercise.supersetGroup) {
        // Find next in superset chain that still has uncompleted sets
        const chain = store.exercises.filter((e) => e.supersetGroup === activeExercise.supersetGroup);
        const curIdx = chain.findIndex((e) => e.id === activeExercise.id);
        for (let i = 1; i <= chain.length; i++) {
          const candidate = chain[(curIdx + i) % chain.length];
          if (candidate.sets.some((s) => !s.completed)) {
            store.setActiveExercise(candidate.id);
            return;
          }
        }
      }
      // All done in superset or no superset — go to next exercise
      if (activeIndex < totalExercises - 1) {
        goToExercise(activeIndex + 1);
      } else {
        handleFinish();
      }
      return;
    }

    // Fill in defaults if empty
    if (!nextSet.weight_kg && nextSet.prev_weight_kg !== null) {
      store.updateSet(activeExercise.id, nextSet.id, 'weight_kg', displayWeight(nextSet.prev_weight_kg));
    }
    if (!nextSet.reps && nextSet.prev_reps !== null) {
      store.updateSet(activeExercise.id, nextSet.id, 'reps', nextSet.prev_reps.toString());
    }

    // Mark set completed
    store.toggleSetCompleted(activeExercise.id, nextSet.id);
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);

    // Superset: rotate to next exercise in chain
    if (activeExercise.supersetGroup) {
      const chain = store.exercises.filter((e) => e.supersetGroup === activeExercise.supersetGroup);
      const curIdx = chain.findIndex((e) => e.id === activeExercise.id);
      const nextInChain = chain[(curIdx + 1) % chain.length];
      if (nextInChain.id !== activeExercise.id) {
        store.setActiveExercise(nextInChain.id);
        // No rest between superset exercises
        return;
      }
    }

    // Normal exercise — start rest timer
    const restTime = store.getRestForExercise(activeExercise.id, restTimerSeconds);
    store.startRestTimer(restTime);
  }

  function handleToggleSet(exerciseId: string, setId: string, wasCompleted: boolean) {
    store.toggleSetCompleted(exerciseId, setId);
    if (!wasCompleted) {
      Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
      const ex = store.exercises.find((e) => e.id === exerciseId);
      if (ex?.supersetGroup) {
        const supersetExercises = store.exercises.filter((e) => e.supersetGroup === ex.supersetGroup);
        const currentIdx = supersetExercises.findIndex((e) => e.id === exerciseId);
        const nextInSuperset = supersetExercises[(currentIdx + 1) % supersetExercises.length];
        if (nextInSuperset && nextInSuperset.id !== exerciseId) {
          store.setActiveExercise(nextInSuperset.id);
          return;
        }
      }
      const restTime = store.getRestForExercise(exerciseId, restTimerSeconds);
      store.startRestTimer(restTime);
    }
  }

  if (!store.isActive) {
    return (
      <SafeAreaView style={[styles.container, { backgroundColor: colors.background, alignItems: 'center', justifyContent: 'center' }]}>
        <Text style={{ color: colors.textSecondary, fontSize: 16 }}>Нет активной тренировки</Text>
        <TouchableOpacity onPress={() => router.back()} style={{ marginTop: 16 }}>
          <Text style={{ color: colors.accent, fontSize: 16 }}>Назад</Text>
        </TouchableOpacity>
      </SafeAreaView>
    );
  }

  function formatTime(seconds: number) {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  function handleFinish() {
    const completedSets = store.exercises.reduce(
      (acc, ex) => acc + ex.sets.filter((s) => s.completed).length, 0
    );
    if (completedSets === 0) {
      Alert.alert('Нет подходов', 'Завершите хотя бы один подход.');
      return;
    }
    Alert.alert('Завершить тренировку?', `Выполнено подходов: ${completedSets}`, [
      { text: 'Отмена', style: 'cancel' },
      { text: 'Завершить', onPress: async () => { await store.finishWorkout(); router.back(); } },
    ]);
  }

  function handleCancel() {
    Alert.alert('Отменить тренировку?', 'Весь прогресс будет потерян.', [
      { text: 'Продолжить', style: 'cancel' },
      { text: 'Отменить', style: 'destructive', onPress: () => { store.cancelWorkout(); router.back(); } },
    ]);
  }

  function handleLinkSuperset() {
    if (!activeExercise) return;
    const idx = store.exercises.findIndex((e) => e.id === activeExercise.id);
    const next = store.exercises[idx + 1];
    if (!next) {
      Alert.alert('Нужно следующее', 'Добавьте упражнение, чтобы создать суперсет.');
      return;
    }
    if (activeExercise.supersetGroup && next.supersetGroup === activeExercise.supersetGroup) return;
    if (activeExercise.supersetGroup) {
      store.linkSuperset([
        ...store.exercises.filter(e => e.supersetGroup === activeExercise.supersetGroup).map(e => e.id),
        next.id,
      ]);
    } else {
      store.linkSuperset([activeExercise.id, next.id]);
    }
  }

  function displayWeight(kg: number | null) {
    if (kg === null) return '';
    if (units === 'lbs') return (kg * 2.205).toFixed(0);
    return kg.toString();
  }

  const unitLabel = units === 'lbs' ? 'LBS' : 'КГ';
  const ssColor = activeExercise?.supersetGroup
    ? SUPERSET_COLORS[(activeExercise.supersetGroup - 1) % SUPERSET_COLORS.length]
    : null;

  // Button state
  const completedCount = activeExercise?.sets.filter((s) => s.completed).length ?? 0;
  const totalSets = activeExercise?.sets.length ?? 0;
  const allDone = completedCount === totalSets && totalSets > 0;
  const isLastExercise = activeIndex >= totalExercises - 1;

  return (
    <SafeAreaView style={[styles.container, { backgroundColor: colors.background }]} edges={['top']}>
      {/* Top bar */}
      <View style={styles.header}>
        <TouchableOpacity onPress={handleCancel} hitSlop={8}>
          <Ionicons name="close" size={24} color={colors.danger} />
        </TouchableOpacity>
        <Text style={[styles.timer, { color: colors.text }]}>{formatTime(elapsed)}</Text>
        <TouchableOpacity onPress={handleFinish} hitSlop={8}>
          <Text style={[styles.finishText, { color: colors.success }]}>Готово</Text>
        </TouchableOpacity>
      </View>

      {/* Rest Timer */}
      {restRemaining > 0 && (
        <View style={[styles.restBanner, { backgroundColor: colors.accent }]}>
          <Text style={styles.restText}>Отдых {formatTime(restRemaining)}</Text>
          <TouchableOpacity onPress={() => store.clearRestTimer()}>
            <Text style={styles.restSkip}>Пропустить</Text>
          </TouchableOpacity>
        </View>
      )}

      {/* Dots navigation */}
      <View style={styles.navRow}>
        {store.exercises.map((ex, i) => {
          const done = ex.sets.every((s) => s.completed);
          const partial = ex.sets.some((s) => s.completed);
          return (
            <TouchableOpacity key={ex.id} onPress={() => goToExercise(i)} hitSlop={4}>
              <View
                style={[
                  styles.dot,
                  {
                    backgroundColor: i === activeIndex ? colors.accent
                      : done ? colors.success
                      : partial ? colors.warning
                      : colors.border,
                  },
                  i === activeIndex && styles.dotActive,
                ]}
              />
            </TouchableOpacity>
          );
        })}
      </View>

      {/* Swipeable exercise area */}
      {activeExercise ? (
        <Animated.View
          style={{ flex: 1, transform: [{ translateX }] }}
          {...panResponder.panHandlers}
        >
          <ScrollView
            style={{ flex: 1 }}
            contentContainerStyle={styles.exerciseContent}
            showsVerticalScrollIndicator={false}
            keyboardShouldPersistTaps="handled"
          >
            {/* Image */}
            {activeExercise.exercise.images?.length > 0 && (
              <View style={styles.imageContainer}>
                <Image
                  source={{ uri: IMAGE_BASE + activeExercise.exercise.images[0] }}
                  style={styles.exerciseImage}
                  resizeMode="cover"
                />
              </View>
            )}

            {/* Name */}
            <View style={styles.exerciseNameRow}>
              {ssColor && (
                <View style={[styles.supersetBadge, { backgroundColor: ssColor + '25' }]}>
                  <Ionicons name="link" size={12} color={ssColor} />
                  <Text style={[styles.supersetBadgeText, { color: ssColor }]}>Суперсет</Text>
                </View>
              )}
              <Text style={[styles.exerciseName, { color: colors.text }]}>
                {activeExercise.exercise.name}
              </Text>
              <Text style={[styles.exerciseCounter, { color: colors.textTertiary }]}>
                {activeIndex + 1} / {totalExercises}
              </Text>
            </View>

            {/* Actions */}
            <View style={styles.actionsRow}>
              <TouchableOpacity
                style={[styles.actionChip, { backgroundColor: colors.surfaceLight }]}
                onPress={() => setEditingRestId(editingRestId === activeExercise.id ? null : activeExercise.id)}
              >
                <Ionicons name="timer-outline" size={15} color={colors.textSecondary} />
                <Text style={[styles.actionChipText, { color: colors.textSecondary }]}>
                  {activeExercise.restSeconds ? `${activeExercise.restSeconds}с` : `${restTimerSeconds}с`}
                </Text>
              </TouchableOpacity>

              <TouchableOpacity
                style={[styles.actionChip, { backgroundColor: ssColor ? ssColor + '25' : colors.surfaceLight }]}
                onPress={handleLinkSuperset}
              >
                <Ionicons name="link" size={15} color={ssColor || colors.textSecondary} />
                <Text style={[styles.actionChipText, { color: ssColor || colors.textSecondary }]}>Суперсет</Text>
              </TouchableOpacity>

              {activeExercise.supersetGroup && (
                <TouchableOpacity
                  style={[styles.actionChip, { backgroundColor: colors.surfaceLight }]}
                  onPress={() => store.unlinkSuperset(activeExercise.id)}
                >
                  <Ionicons name="unlink" size={15} color={colors.textTertiary} />
                </TouchableOpacity>
              )}

              <TouchableOpacity
                style={[styles.actionChip, { backgroundColor: colors.danger + '15' }]}
                onPress={() => {
                  Alert.alert('Удалить?', activeExercise.exercise.name, [
                    { text: 'Нет', style: 'cancel' },
                    { text: 'Удалить', style: 'destructive', onPress: () => store.removeExercise(activeExercise.id) },
                  ]);
                }}
              >
                <Ionicons name="trash-outline" size={15} color={colors.danger} />
              </TouchableOpacity>
            </View>

            {/* Rest picker */}
            {editingRestId === activeExercise.id && (
              <View style={styles.restPicker}>
                {[0, 30, 45, 60, 90, 120, 180, 300].map((s) => {
                  const isSelected = s === 0 ? activeExercise.restSeconds === null : activeExercise.restSeconds === s;
                  return (
                    <TouchableOpacity
                      key={s}
                      style={[styles.restPickerBtn, { backgroundColor: isSelected ? colors.accent : colors.surfaceLight }]}
                      onPress={() => { store.setExerciseRest(activeExercise.id, s === 0 ? null : s); setEditingRestId(null); }}
                    >
                      <Text style={[styles.restPickerText, { color: isSelected ? '#fff' : colors.textSecondary }]}>
                        {s === 0 ? 'Базовый' : `${s}с`}
                      </Text>
                    </TouchableOpacity>
                  );
                })}
              </View>
            )}

            {/* Sets */}
            <View style={[styles.setsCard, { backgroundColor: colors.surface }]}>
              <View style={styles.setHeaderRow}>
                <Text style={[styles.setHeaderText, { color: colors.textTertiary, width: 36 }]}>№</Text>
                <Text style={[styles.setHeaderText, { color: colors.textTertiary, flex: 1 }]}>ПРЕД.</Text>
                <Text style={[styles.setHeaderText, { color: colors.textTertiary, flex: 1 }]}>{unitLabel}</Text>
                <Text style={[styles.setHeaderText, { color: colors.textTertiary, flex: 1 }]}>ПОВТ.</Text>
                <View style={{ width: 44 }} />
              </View>

              {activeExercise.sets.map((s, si) => (
                <View key={s.id} style={[styles.setRow, s.completed && { backgroundColor: colors.success + '12' }]}>
                  <Text style={[styles.setNumber, { color: colors.textSecondary }]}>
                    {s.set_type === 'warmup' ? 'Р' : si + 1}
                  </Text>
                  <Text style={[styles.prevText, { color: colors.textTertiary }]}>
                    {s.prev_weight_kg !== null && s.prev_reps !== null
                      ? `${displayWeight(s.prev_weight_kg)} × ${s.prev_reps}`
                      : '-'}
                  </Text>
                  <TextInput
                    style={[styles.setInput, { color: colors.text, backgroundColor: colors.surfaceLight }]}
                    value={s.weight_kg}
                    onChangeText={(v) => store.updateSet(activeExercise.id, s.id, 'weight_kg', v)}
                    keyboardType="decimal-pad"
                    placeholder={s.prev_weight_kg !== null ? displayWeight(s.prev_weight_kg) : '0'}
                    placeholderTextColor={colors.textTertiary}
                  />
                  <TextInput
                    style={[styles.setInput, { color: colors.text, backgroundColor: colors.surfaceLight }]}
                    value={s.reps}
                    onChangeText={(v) => store.updateSet(activeExercise.id, s.id, 'reps', v)}
                    keyboardType="number-pad"
                    placeholder={s.prev_reps?.toString() || '0'}
                    placeholderTextColor={colors.textTertiary}
                  />
                  <TouchableOpacity
                    style={[styles.checkButton, { backgroundColor: s.completed ? colors.success : colors.surfaceLight }]}
                    onPress={() => handleToggleSet(activeExercise.id, s.id, s.completed)}
                  >
                    <Ionicons name="checkmark" size={20} color={s.completed ? '#fff' : colors.textTertiary} />
                  </TouchableOpacity>
                </View>
              ))}

              <TouchableOpacity
                style={[styles.addSetBtn, { backgroundColor: colors.surfaceLight }]}
                onPress={() => store.addSet(activeExercise.id)}
              >
                <Ionicons name="add" size={16} color={colors.accent} />
                <Text style={[styles.addSetText, { color: colors.accent }]}>Подход</Text>
              </TouchableOpacity>
            </View>

            <View style={{ height: 90 }} />
          </ScrollView>
        </Animated.View>
      ) : (
        <View style={{ flex: 1, justifyContent: 'center', alignItems: 'center', padding: 32 }}>
          <Text style={{ color: colors.textSecondary, fontSize: 16, textAlign: 'center' }}>
            Нет упражнений
          </Text>
        </View>
      )}

      {/* Bottom "Сделал" button — sticky */}
      {activeExercise && (
        <View style={[styles.bottomBar, { backgroundColor: colors.background, paddingBottom: insets.bottom + 8 }]}>
          <TouchableOpacity
            style={[
              styles.doneButton,
              { backgroundColor: allDone ? (isLastExercise ? colors.success : colors.accent) : colors.success },
            ]}
            onPress={handleDone}
            activeOpacity={0.7}
          >
            <Text style={styles.doneButtonText}>
              {allDone
                ? (isLastExercise ? 'Завершить тренировку' : 'Следующее упражнение →')
                : `Сделал  (${completedCount}/${totalSets})`
              }
            </Text>
          </TouchableOpacity>
        </View>
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  header: {
    flexDirection: 'row', justifyContent: 'space-between',
    alignItems: 'center', paddingHorizontal: 16, paddingVertical: 8,
  },
  timer: { fontSize: 20, fontWeight: '700', fontVariant: ['tabular-nums'] },
  finishText: { fontSize: 16, fontWeight: '700' },
  restBanner: {
    flexDirection: 'row', justifyContent: 'space-between',
    alignItems: 'center', paddingHorizontal: 16, paddingVertical: 10,
  },
  restText: { color: '#fff', fontSize: 16, fontWeight: '700', fontVariant: ['tabular-nums'] },
  restSkip: { color: '#fff', fontSize: 14, fontWeight: '500', opacity: 0.8 },
  navRow: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    paddingVertical: 6, gap: 6, flexWrap: 'wrap', paddingHorizontal: 16,
  },
  dot: { width: 8, height: 8, borderRadius: 4 },
  dotActive: { width: 12, height: 12, borderRadius: 6 },
  exerciseContent: { paddingHorizontal: 16, paddingTop: 4 },
  imageContainer: {
    borderRadius: 12, overflow: 'hidden', marginBottom: 12,
    height: 160, backgroundColor: '#1a1a1a',
  },
  exerciseImage: { width: '100%', height: '100%' },
  exerciseNameRow: { marginBottom: 10 },
  supersetBadge: {
    flexDirection: 'row', alignItems: 'center', gap: 4,
    alignSelf: 'flex-start', paddingHorizontal: 8, paddingVertical: 3,
    borderRadius: 6, marginBottom: 6,
  },
  supersetBadgeText: { fontSize: 11, fontWeight: '600' },
  exerciseName: { fontSize: 22, fontWeight: '800' },
  exerciseCounter: { fontSize: 13, marginTop: 2 },
  actionsRow: { flexDirection: 'row', gap: 8, marginBottom: 10, flexWrap: 'wrap' },
  actionChip: {
    flexDirection: 'row', alignItems: 'center', gap: 4,
    paddingHorizontal: 10, paddingVertical: 7, borderRadius: 8,
  },
  actionChipText: { fontSize: 13, fontWeight: '500' },
  restPicker: { flexDirection: 'row', flexWrap: 'wrap', gap: 6, marginBottom: 12 },
  restPickerBtn: { paddingHorizontal: 12, paddingVertical: 7, borderRadius: 8 },
  restPickerText: { fontSize: 13, fontWeight: '600' },
  setsCard: { borderRadius: 12, padding: 12, marginBottom: 12 },
  setHeaderRow: {
    flexDirection: 'row', alignItems: 'center', paddingVertical: 4, paddingHorizontal: 2,
  },
  setHeaderText: { fontSize: 11, fontWeight: '600', textAlign: 'center' },
  setRow: {
    flexDirection: 'row', alignItems: 'center', paddingVertical: 5,
    paddingHorizontal: 2, borderRadius: 8, marginBottom: 3,
  },
  setNumber: { width: 36, textAlign: 'center', fontSize: 15, fontWeight: '700' },
  prevText: { flex: 1, textAlign: 'center', fontSize: 13 },
  setInput: {
    flex: 1, height: 44, borderRadius: 8, textAlign: 'center',
    fontSize: 17, fontWeight: '600', marginHorizontal: 3,
  },
  checkButton: {
    width: 44, height: 44, borderRadius: 8,
    justifyContent: 'center', alignItems: 'center', marginLeft: 4,
  },
  addSetBtn: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    paddingVertical: 10, borderRadius: 8, gap: 4, marginTop: 8,
  },
  addSetText: { fontSize: 14, fontWeight: '600' },
  bottomBar: {
    paddingHorizontal: 16, paddingTop: 8,
    borderTopWidth: StyleSheet.hairlineWidth, borderTopColor: '#2A2A2A',
  },
  doneButton: {
    paddingVertical: 16, borderRadius: 12, alignItems: 'center',
  },
  doneButtonText: { color: '#fff', fontSize: 17, fontWeight: '700' },
});
