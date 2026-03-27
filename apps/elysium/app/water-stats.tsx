import React, { useMemo, useState, useCallback } from 'react';
import { View, ScrollView, StyleSheet, Text, Pressable } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { LinearGradient } from 'expo-linear-gradient';
import { format, subDays } from 'date-fns';
import { ru } from 'date-fns/locale';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, fonts, cardRadius } from '@/theme';
import { useWaterStore } from '@/stores/water-store';
import { WeekChart } from '@/components/WeekChart';

const PAD = spacing.lg;

export default function WaterStatsScreen() {
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const entriesByDate = useWaterStore((s) => s.entriesByDate);
  const goal = useWaterStore((s) => s.goal);
  const [selectedDate, setSelectedDate] = useState<string | null>(null);

  const waterByDate = useMemo(() => {
    const map: Record<string, number> = {};
    for (const [date, entries] of Object.entries(entriesByDate)) {
      map[date] = entries.reduce((sum, e) => sum + e.amount, 0);
    }
    return map;
  }, [entriesByDate]);

  const today = format(new Date(), 'yyyy-MM-dd');
  const viewDate = selectedDate ?? today;
  const viewEntries = entriesByDate[viewDate] ?? [];
  const viewTotal = viewEntries.reduce((sum, e) => sum + e.amount, 0);
  const viewCount = viewEntries.length;
  const viewIsToday = viewDate === today;

  // Averages
  const avg = useMemo(() => {
    const allDays = Object.entries(entriesByDate).filter(([, e]) => e.length > 0);
    if (allDays.length === 0) return { ml: 0, days: 0 };
    const totalMl = allDays.reduce((a, [, e]) => a + e.reduce((s, x) => s + x.amount, 0), 0);
    return { ml: totalMl / allDays.length, days: allDays.length };
  }, [entriesByDate]);

  // Best day
  const bestDay = useMemo(() => {
    let best = 0;
    for (const entries of Object.values(entriesByDate)) {
      const total = entries.reduce((s, e) => s + e.amount, 0);
      if (total > best) best = total;
    }
    return best;
  }, [entriesByDate]);

  // Streak
  const streak = useMemo(() => {
    let count = 0;
    for (let i = 0; i <= 30; i++) {
      const date = format(subDays(new Date(), i), 'yyyy-MM-dd');
      const total = (entriesByDate[date] ?? []).reduce((s, e) => s + e.amount, 0);
      if (total >= goal) count++;
      else break;
    }
    return count;
  }, [entriesByDate, goal]);

  const handleSelectDate = useCallback((date: string | null) => {
    setSelectedDate(date);
  }, []);

  const dayLabel = viewIsToday && !selectedDate
    ? 'Сегодня'
    : format(new Date(viewDate + 'T12:00:00'), 'd MMMM, EEEE', { locale: ru });

  const ratio = goal > 0 ? viewTotal / goal : 0;

  return (
    <View style={styles.root}>
      <LinearGradient
        colors={['#1A3A5C', '#132E4A', '#0D1D2E', colors.bg.primary]}
        locations={[0, 0.3, 0.65, 1]}
        start={{ x: 0, y: 0 }}
        end={{ x: 0.5, y: 1 }}
        style={styles.gradientBg}
      />
      <ScrollView style={styles.scroll} contentContainerStyle={[styles.scrollContent, { paddingTop: insets.top + 12 }]} showsVerticalScrollIndicator={false}>
        <View style={styles.chartSection}>
          <View style={styles.header}>
            <Pressable onPress={() => router.back()} hitSlop={12} style={styles.backBtn}>
              <Ionicons name="chevron-back" size={24} color={colors.water} />
            </Pressable>
            <Text style={styles.title}>Вода</Text>
            <View style={{ width: 36 }} />
          </View>

          <WeekChart
            dataByDate={waterByDate}
            goal={goal}
            color={colors.water}
            onSelectDate={handleSelectDate}
          />
        </View>

        <View style={styles.content}>
        {/* Day detail */}
        <View style={styles.dayCard}>
          <Text style={styles.dayLabel}>{dayLabel}</Text>
          <Text style={styles.dayValue}>{viewTotal} <Text style={styles.dayUnit}>мл</Text></Text>

          {/* Progress bar */}
          <View style={styles.progressRow}>
            <View style={styles.progressTrack}>
              <View style={[styles.progressFill, { width: `${Math.min(ratio * 100, 100)}%` }]} />
            </View>
            <Text style={styles.progressPercent}>{Math.round(ratio * 100)}%</Text>
          </View>

          {/* Mini stats */}
          <View style={styles.miniRow}>
            <View style={styles.miniItem}>
              <Text style={styles.miniValue}>{viewCount}</Text>
              <Text style={styles.miniLabel}>приёмов</Text>
            </View>
            <View style={styles.miniItem}>
              <Text style={styles.miniValue}>{goal}</Text>
              <Text style={styles.miniLabel}>цель, мл</Text>
            </View>
            <View style={styles.miniItem}>
              <Text style={styles.miniValue}>{Math.max(goal - viewTotal, 0)}</Text>
              <Text style={styles.miniLabel}>осталось, мл</Text>
            </View>
          </View>
        </View>

        {/* Average summary */}
        {avg.days > 0 && (
          <View style={styles.avgCard}>
            <Text style={styles.avgTitle}>Среднее за {avg.days} дн.</Text>
            <View style={styles.avgRow}>
              <View style={styles.avgItem}>
                <Text style={[styles.avgValue, { color: colors.water }]}>{Math.round(avg.ml)}</Text>
                <Text style={styles.avgLabel}>мл/день</Text>
              </View>
              <View style={styles.avgDivider} />
              <View style={styles.avgItem}>
                <Text style={styles.avgValue}>{bestDay}</Text>
                <Text style={styles.avgLabel}>лучший</Text>
              </View>
              <View style={styles.avgDivider} />
              <View style={styles.avgItem}>
                <Text style={styles.avgValue}>{streak}</Text>
                <Text style={styles.avgLabel}>серия</Text>
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

  // Day card
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
  dayValue: {
    fontSize: fontSize.xxl,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
    marginBottom: spacing.md,
  },
  dayUnit: {
    fontSize: fontSize.md,
    fontFamily: fonts.regular,
    color: colors.text.muted,
  },
  progressRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.sm,
    marginBottom: spacing.lg,
  },
  progressTrack: {
    flex: 1,
    height: 4,
    borderRadius: 2,
    backgroundColor: 'rgba(255,255,255,0.06)',
  },
  progressFill: {
    height: 4,
    borderRadius: 2,
    backgroundColor: colors.water,
  },
  progressPercent: {
    fontSize: fontSize.xs,
    fontFamily: fonts.mono,
    color: colors.text.muted,
    width: 36,
    textAlign: 'right',
  },
  miniRow: {
    flexDirection: 'row',
  },
  miniItem: {
    flex: 1,
    alignItems: 'center',
  },
  miniValue: {
    fontSize: fontSize.md,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
  },
  miniLabel: {
    fontSize: fontSize.xs,
    fontFamily: fonts.regular,
    color: colors.text.muted,
    marginTop: 2,
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
