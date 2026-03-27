import React, { useState, useMemo, useCallback } from 'react';
import { View, ScrollView, StyleSheet, Text, Pressable, Modal } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { format, addDays, subDays } from 'date-fns';
import { ru } from 'date-fns/locale';
import { Ionicons } from '@expo/vector-icons';
import { colors, spacing, fonts, fontSize } from '@/theme';
import { useNutritionStore } from '@/stores/nutrition-store';
import { CalorieHero } from '@/components/CalorieHero';
import { MacroCard } from '@/components/MacroCard';
import { MealCard } from '@/components/MealCard';
import { QuantityPicker } from '@/components/QuantityPicker';
import { sumMacros } from '@/utils/macros';
import type { MealType, MealEntry } from '@/types/nutrition';

const PAD = spacing.lg;
const EMPTY_ENTRIES: MealEntry[] = [];

export default function DiaryScreen() {
  const router = useRouter();
  const insets = useSafeAreaInsets();

  const selectedDate = useNutritionStore((s) => s.selectedDate);
  const entriesRaw = useNutritionStore((s) => s.entriesByDate[s.selectedDate]);
  const entries = entriesRaw ?? EMPTY_ENTRIES;
  const goals = useNutritionStore((s) => s.goals);
  const setSelectedDate = useNutritionStore((s) => s.setSelectedDate);
  const updateEntryQuantity = useNutritionStore((s) => s.updateEntryQuantity);
  const removeEntry = useNutritionStore((s) => s.removeEntry);

  const [editingEntry, setEditingEntry] = useState<MealEntry | null>(null);

  // Memoize computations
  const totals = useMemo(() => sumMacros(entries), [entries]);
  const byMeal = useMemo(() => ({
    breakfast: entries.filter((e) => e.mealType === 'breakfast'),
    lunch: entries.filter((e) => e.mealType === 'lunch'),
    dinner: entries.filter((e) => e.mealType === 'dinner'),
    snack: entries.filter((e) => e.mealType === 'snack'),
  }), [entries]);

  const dateObj = new Date(selectedDate + 'T00:00:00');
  const isToday = selectedDate === format(new Date(), 'yyyy-MM-dd');

  const goBack = useCallback(() => {
    const d = new Date(selectedDate + 'T00:00:00');
    setSelectedDate(format(subDays(d, 1), 'yyyy-MM-dd'));
  }, [selectedDate, setSelectedDate]);

  const goForward = useCallback(() => {
    const d = new Date(selectedDate + 'T00:00:00');
    setSelectedDate(format(addDays(d, 1), 'yyyy-MM-dd'));
  }, [selectedDate, setSelectedDate]);

  const handleAddMeal = useCallback((mealType: MealType) => {
    router.push({ pathname: '/add-food', params: { mealType } });
  }, [router]);

  const macros = useMemo(() => [
    { label: 'Углеводы', current: totals.carbs, goal: goals.carbs, color: colors.macro.carbs },
    { label: 'Белки', current: totals.protein, goal: goals.protein, color: colors.macro.protein },
    { label: 'Жиры', current: totals.fat, goal: goals.fat, color: colors.macro.fat },
  ], [totals, goals]);

  return (
    <View style={styles.root}>
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={[styles.scrollContent, { paddingTop: insets.top + 12 }]}
        showsVerticalScrollIndicator={false}
      >
        <CalorieHero
          eaten={totals.calories}
          goal={goals.calories}
          onPress={() => router.push('/stats')}
        />

        <View style={{ height: spacing.xs }} />

        <MacroCard macros={macros} />

        <View style={styles.dateRow}>
          <Pressable onPress={goBack} hitSlop={16}>
            <Ionicons name="chevron-back" size={20} color={colors.text.muted} />
          </Pressable>
          <Pressable
            style={styles.dateCenter}
            onPress={() => setSelectedDate(format(new Date(), 'yyyy-MM-dd'))}
          >
            <Text style={styles.dateText}>
              {isToday
                ? 'Сегодня'
                : format(dateObj, 'd MMM, EEEE', { locale: ru })}
            </Text>
          </Pressable>
          <Pressable onPress={goForward} hitSlop={16}>
            <Ionicons name="chevron-forward" size={20} color={colors.text.muted} />
          </Pressable>
        </View>

        {(['breakfast', 'lunch', 'dinner', 'snack'] as MealType[]).map((type) => (
          <MealCard
            key={type}
            mealType={type}
            entries={byMeal[type]}
            onAdd={handleAddMeal}
            onEdit={setEditingEntry}
            onRemove={removeEntry}
          />
        ))}

        <View style={{ height: 100 }} />
      </ScrollView>

      <Modal visible={!!editingEntry} animationType="slide" presentationStyle="pageSheet">
        <SafeAreaView style={styles.modalRoot} edges={['top']}>
          {editingEntry && (
            <QuantityPicker
              food={editingEntry.foodItem}
              mealType={editingEntry.mealType}
              initialQuantity={editingEntry.quantity}
              confirmLabel="Сохранить"
              onConfirm={(qty, _meal) => {
                updateEntryQuantity(editingEntry.id, qty);
                setEditingEntry(null);
              }}
              onBack={() => setEditingEntry(null)}
            />
          )}
        </SafeAreaView>
      </Modal>
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: colors.bg.primary,
  },
  scroll: { flex: 1 },
  scrollContent: {},
  dateRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: spacing.lg,
    paddingHorizontal: PAD,
  },
  dateCenter: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  dateText: {
    fontSize: fontSize.md,
    fontFamily: fonts.semiBold,
    color: colors.text.primary,
  },
  modalRoot: {
    flex: 1,
    backgroundColor: colors.bg.primary,
  },
});
