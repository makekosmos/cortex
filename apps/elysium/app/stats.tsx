import React, { useMemo, useState, useCallback } from 'react';
import { View, ScrollView, StyleSheet, Text, Pressable } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { LinearGradient } from 'expo-linear-gradient';
import { format } from 'date-fns';
import { ru } from 'date-fns/locale';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, fonts, cardRadius } from '@/theme';
import { useNutritionStore } from '@/stores/nutrition-store';
import { sumMacros } from '@/utils/macros';
import { WeekChart } from '@/components/WeekChart';

const PAD = spacing.lg;

export default function StatsScreen() {
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const entriesByDate = useNutritionStore((s) => s.entriesByDate);
  const goals = useNutritionStore((s) => s.goals);
  const [selectedDate, setSelectedDate] = useState<string | null>(null);

  const caloriesByDate = useMemo(() => {
    const map: Record<string, number> = {};
    for (const [date, entries] of Object.entries(entriesByDate)) {
      map[date] = sumMacros(entries).calories;
    }
    return map;
  }, [entriesByDate]);

  const viewDate = selectedDate ?? format(new Date(), 'yyyy-MM-dd');
  const viewEntries = entriesByDate[viewDate] ?? [];
  const viewTotals = useMemo(() => sumMacros(viewEntries), [viewEntries]);
  const viewIsToday = viewDate === format(new Date(), 'yyyy-MM-dd');

  // Averages
  const avg = useMemo(() => {
    const allDays = Object.entries(entriesByDate).filter(([, e]) => e.length > 0);
    if (allDays.length === 0) return { calories: 0, protein: 0, fat: 0, carbs: 0, days: 0 };
    const totals = allDays.map(([, e]) => sumMacros(e));
    const n = totals.length;
    return {
      calories: totals.reduce((a, t) => a + t.calories, 0) / n,
      protein: totals.reduce((a, t) => a + t.protein, 0) / n,
      fat: totals.reduce((a, t) => a + t.fat, 0) / n,
      carbs: totals.reduce((a, t) => a + t.carbs, 0) / n,
      days: n,
    };
  }, [entriesByDate]);

  const handleSelectDate = useCallback((date: string | null) => {
    setSelectedDate(date);
  }, []);

  const dayLabel = viewIsToday && !selectedDate
    ? 'Сегодня'
    : format(new Date(viewDate + 'T12:00:00'), 'd MMMM, EEEE', { locale: ru });

  return (
    <View style={styles.root}>
      <LinearGradient
        colors={['#2A1F5E', '#1E1545', '#131025', colors.bg.primary]}
        locations={[0, 0.3, 0.65, 1]}
        start={{ x: 0, y: 0 }}
        end={{ x: 0.5, y: 1 }}
        style={styles.gradientBg}
      />
      <ScrollView style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingTop: insets.top + 12 }]} showsVerticalScrollIndicator={false}>
        <View style={styles.chartSection}>
          <View style={styles.header}>
            <Pressable onPress={() => router.back()} hitSlop={12} style={styles.backBtn}>
              <Ionicons name="chevron-back" size={24} color={colors.accent} />
            </Pressable>
            <Text style={styles.title}>Калории</Text>
            <View style={{ width: 36 }} />
          </View>

          <WeekChart
            dataByDate={caloriesByDate}
            goal={goals.calories}
            color={colors.accent}
            onSelectDate={handleSelectDate}
          />
        </View>

        <View style={styles.content}>
        {/* Day detail */}
        <View style={styles.dayCard}>
          <Text style={styles.dayLabel}>{dayLabel}</Text>
          <Text style={styles.dayCalories}>{Math.round(viewTotals.calories)} <Text style={styles.dayUnit}>ккал</Text></Text>

          {/* Macros */}
          {([
            { key: 'protein' as const, label: 'Белки', color: colors.macro.protein },
            { key: 'fat' as const, label: 'Жиры', color: colors.macro.fat },
            { key: 'carbs' as const, label: 'Углеводы', color: colors.macro.carbs },
          ]).map(({ key, label, color }) => {
            const current = Math.round(viewTotals[key]);
            const goal = goals[key];
            const ratio = goal > 0 ? Math.min(current / goal, 1.3) : 0;
            return (
              <View key={key} style={styles.macroRow}>
                <View style={styles.macroLabelRow}>
                  <View style={[styles.macroDot, { backgroundColor: color }]} />
                  <Text style={styles.macroLabel}>{label}</Text>
                  <Text style={styles.macroValues}>{current} / {goal}г</Text>
                </View>
                <View style={styles.progressTrack}>
                  <View style={[styles.progressFill, { width: `${Math.min(ratio * 100, 100)}%`, backgroundColor: color }]} />
                </View>
              </View>
            );
          })}
        </View>

        {/* Average summary */}
        {avg.days > 0 && (
          <View style={styles.avgCard}>
            <Text style={styles.avgTitle}>Среднее за {avg.days} дн.</Text>
            <View style={styles.avgRow}>
              <View style={styles.avgItem}>
                <Text style={styles.avgValue}>{Math.round(avg.calories)}</Text>
                <Text style={styles.avgLabel}>ккал</Text>
              </View>
              <View style={styles.avgDivider} />
              <View style={styles.avgItem}>
                <Text style={[styles.avgValue, { color: colors.macro.protein }]}>{Math.round(avg.protein)}</Text>
                <Text style={styles.avgLabel}>белки</Text>
              </View>
              <View style={styles.avgDivider} />
              <View style={styles.avgItem}>
                <Text style={[styles.avgValue, { color: colors.macro.fat }]}>{Math.round(avg.fat)}</Text>
                <Text style={styles.avgLabel}>жиры</Text>
              </View>
              <View style={styles.avgDivider} />
              <View style={styles.avgItem}>
                <Text style={[styles.avgValue, { color: colors.macro.carbs }]}>{Math.round(avg.carbs)}</Text>
                <Text style={styles.avgLabel}>углеводы</Text>
              </View>
            </View>
          </View>
        )}

        <View style={{ height: 40 }} />
      </View>
    </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: colors.bg.primary },
  gradientBg: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    height: 500,
  },
  scroll: { flex: 1 },
  scrollContent: { flexGrow: 1 },
  chartSection: {
    paddingHorizontal: PAD,
    marginBottom: spacing.lg,
  },
  content: {
    paddingHorizontal: PAD,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: spacing.lg,
  },
  backBtn: {
    width: 36,
    height: 36,
    borderRadius: 12,
    backgroundColor: 'rgba(255,255,255,0.08)',
    justifyContent: 'center',
    alignItems: 'center',
  },
  title: {
    fontFamily: fonts.bold,
    fontSize: fontSize.lg,
    color: colors.text.primary,
  },

  // Day detail card
  dayCard: {
    backgroundColor: colors.bg.card,
    borderRadius: cardRadius,
    padding: spacing.lg,
    marginBottom: spacing.sm,
  },
  dayLabel: {
    fontSize: fontSize.sm,
    fontFamily: fonts.regular,
    color: colors.text.secondary,
    textTransform: 'capitalize',
    marginBottom: 4,
  },
  dayCalories: {
    fontSize: fontSize.xxl,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
    marginBottom: spacing.lg,
  },
  dayUnit: {
    fontSize: fontSize.md,
    fontFamily: fonts.regular,
    color: colors.text.muted,
  },
  macroRow: {
    marginBottom: spacing.md,
  },
  macroLabelRow: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 6,
  },
  macroDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    marginRight: 8,
  },
  macroLabel: {
    flex: 1,
    fontSize: fontSize.sm,
    fontFamily: fonts.regular,
    color: colors.text.secondary,
  },
  macroValues: {
    fontSize: fontSize.sm,
    fontFamily: fonts.mono,
    color: colors.text.muted,
  },
  progressTrack: {
    height: 4,
    borderRadius: 2,
    backgroundColor: 'rgba(255,255,255,0.06)',
    marginLeft: 16,
  },
  progressFill: {
    height: 4,
    borderRadius: 2,
  },

  // Average card
  avgCard: {
    backgroundColor: colors.bg.card,
    borderRadius: cardRadius,
    padding: spacing.lg,
  },
  avgTitle: {
    fontSize: fontSize.sm,
    fontFamily: fonts.regular,
    color: colors.text.secondary,
    marginBottom: spacing.md,
  },
  avgRow: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  avgItem: {
    flex: 1,
    alignItems: 'center',
  },
  avgValue: {
    fontSize: fontSize.lg,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
  },
  avgLabel: {
    fontSize: fontSize.xs,
    fontFamily: fonts.regular,
    color: colors.text.muted,
    marginTop: 2,
  },
  avgDivider: {
    width: 1,
    height: 28,
    backgroundColor: colors.separator,
  },
});
