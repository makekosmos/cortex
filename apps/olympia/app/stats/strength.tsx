import { useState, useEffect, useMemo } from 'react';
import {
  View, Text, StyleSheet, ScrollView, TouchableOpacity, Alert,
} from 'react-native';
import { Stack } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useThemeColor } from '@/lib/useThemeColor';
import { useSettingsStore } from '@/lib/stores/settings-store';
import { getDailyStats } from '@/lib/database';

type Period = 'week' | 'month' | 'year';
type Metric = 'volume' | 'duration';

export default function StrengthStatsScreen() {
  const colors = useThemeColor();
  const units = useSettingsStore((s) => s.units);
  const [period, setPeriod] = useState<Period>('week');
  const [metric, setMetric] = useState<Metric>('volume');
  const [data, setData] = useState<{ date: string; volume: number; duration: number; count: number }[]>([]);

  useEffect(() => {
    loadData(period);
  }, [period]);

  async function loadData(p: Period) {
    const days = p === 'week' ? 7 : p === 'month' ? 30 : 365;
    const d = await getDailyStats(days);
    setData(d);
  }

  function formatVolume(kg: number) {
    if (units === 'lbs') kg = kg * 2.205;
    if (kg >= 1000000) return `${(kg / 1000000).toFixed(1)}M`;
    if (kg >= 1000) return `${(kg / 1000).toFixed(1)}K`;
    return kg.toFixed(0);
  }

  function formatDuration(seconds: number) {
    if (!seconds) return '0м';
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    if (h > 0) return `${h}ч ${m}м`;
    return `${m}м`;
  }

  function formatShortDate(iso: string) {
    return new Date(iso).toLocaleDateString('ru-RU', { day: 'numeric', month: 'short' });
  }

  const MONTH_NAMES_SHORT = ['Янв', 'Фев', 'Мар', 'Апр', 'Май', 'Июн', 'Июл', 'Авг', 'Сен', 'Окт', 'Ноя', 'Дек'];

  // Fill all days/months in period
  const chartBars = useMemo(() => {
    const dataMap = new Map(data.map((d) => [d.date, d]));

    if (period === 'year') {
      // Group by month — 12 bars
      const months: { label: string; volume: number; duration: number; count: number }[] = [];
      const today = new Date();
      for (let i = 11; i >= 0; i--) {
        const d = new Date(today.getFullYear(), today.getMonth() - i, 1);
        const year = d.getFullYear();
        const month = d.getMonth();
        const prefix = `${year}-${String(month + 1).padStart(2, '0')}`;
        let volume = 0, duration = 0, count = 0;
        for (const [date, entry] of dataMap) {
          if (date.startsWith(prefix)) {
            volume += entry.volume;
            duration += entry.duration;
            count += entry.count;
          }
        }
        months.push({ label: MONTH_NAMES_SHORT[month], volume, duration, count });
      }
      return months;
    }

    // Week or month — by day
    const days = period === 'week' ? 7 : 30;
    const result: { label: string; volume: number; duration: number; count: number }[] = [];
    const today = new Date();
    for (let i = days - 1; i >= 0; i--) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const dateStr = d.toISOString().split('T')[0];
      const entry = dataMap.get(dateStr);
      result.push({
        label: String(d.getDate()),
        volume: entry?.volume || 0,
        duration: entry?.duration || 0,
        count: entry?.count || 0,
      });
    }
    return result;
  }, [data, period]);

  const totalVolume = data.reduce((a, d) => a + d.volume, 0);
  const totalDuration = data.reduce((a, d) => a + d.duration, 0);
  const totalWorkouts = data.reduce((a, d) => a + d.count, 0);
  const avgVolume = totalWorkouts > 0 ? totalVolume / totalWorkouts : 0;
  const avgDuration = totalWorkouts > 0 ? totalDuration / totalWorkouts : 0;
  const maxBarValue = Math.max(...chartBars.map((d) => metric === 'volume' ? d.volume : d.duration), 1);

  return (
    <>
      <Stack.Screen
        options={{
          title: '',
          headerStyle: { backgroundColor: '#160808' },
          headerTintColor: '#EF4444',
        }}
      />
      <ScrollView style={styles.container}>
        {/* Header */}
        <View style={styles.header}>
          <View style={styles.badge}>
            <Ionicons name="barbell" size={14} color="#EF4444" />
            <Text style={styles.badgeText}>СИЛОВЫЕ</Text>
          </View>
          <Text style={styles.mainTitle}>Статистика</Text>
        </View>

        {/* Period selector */}
        <View style={styles.periodRow}>
          {(['week', 'month', 'year'] as Period[]).map((p) => (
            <TouchableOpacity
              key={p}
              style={[styles.periodBtn, period === p && styles.periodBtnActive]}
              onPress={() => setPeriod(p)}
            >
              <Text style={[styles.periodBtnText, period === p && styles.periodBtnTextActive]}>
                {p === 'week' ? 'Неделя' : p === 'month' ? 'Месяц' : 'Год'}
              </Text>
            </TouchableOpacity>
          ))}
        </View>

        {/* Metric toggle */}
        <View style={styles.metricRow}>
          {(['volume', 'duration'] as Metric[]).map((m) => (
            <TouchableOpacity
              key={m}
              style={[styles.metricBtn, metric === m && styles.metricBtnActive]}
              onPress={() => setMetric(m)}
            >
              <Ionicons
                name={m === 'volume' ? 'barbell-outline' : 'time-outline'}
                size={14}
                color={metric === m ? '#EF4444' : '#666'}
              />
              <Text style={[styles.metricBtnText, metric === m && styles.metricBtnTextActive]}>
                {m === 'volume' ? 'Тоннаж' : 'Время'}
              </Text>
            </TouchableOpacity>
          ))}
        </View>

        {/* Summary cards */}
        <View style={styles.summaryRow}>
          <View style={styles.summaryCard}>
            <Text style={styles.summaryNumber}>
              {metric === 'volume' ? `${formatVolume(totalVolume)} ${units}` : formatDuration(totalDuration)}
            </Text>
            <Text style={styles.summaryLabel}>Всего</Text>
          </View>
          <View style={styles.summaryCard}>
            <Text style={styles.summaryNumber}>{totalWorkouts}</Text>
            <Text style={styles.summaryLabel}>Тренировок</Text>
          </View>
          <View style={styles.summaryCard}>
            <Text style={styles.summaryNumber}>
              {metric === 'volume' ? `${formatVolume(avgVolume)}` : formatDuration(avgDuration)}
            </Text>
            <Text style={styles.summaryLabel}>В среднем</Text>
          </View>
        </View>

        {/* Bar chart */}
        <View style={styles.chartCard}>
          <View style={styles.chart}>
            {chartBars.map((d, i) => {
              const val = metric === 'volume' ? d.volume : d.duration;
              const hasData = val > 0;
              const h = hasData ? Math.max(6, (val / maxBarValue) * 120) : 2;
              return (
                <TouchableOpacity
                  key={i}
                  style={styles.barWrap}
                  onPress={() => {
                    if (!hasData) return;
                    const info = metric === 'volume'
                      ? `${formatVolume(d.volume)} ${units}`
                      : formatDuration(d.duration);
                    Alert.alert(d.label, `${info}\n${d.count} тренировок`);
                  }}
                  activeOpacity={hasData ? 0.7 : 1}
                >
                  <View style={[
                    styles.bar,
                    { height: h },
                    !hasData && { backgroundColor: '#2a1515', opacity: 0.5 },
                  ]} />
                  {chartBars.length <= 14 && (
                    <Text style={styles.barLabel}>{d.label}</Text>
                  )}
                </TouchableOpacity>
              );
            })}
          </View>
          {/* Month labels for year view */}
          {period === 'year' && (
            <View style={styles.monthLabelsRow}>
              {chartBars.map((d, i) => (
                <Text key={i} style={styles.monthLabel}>{d.label}</Text>
              ))}
            </View>
          )}
        </View>

        {/* Day list — only days with data */}
        <View style={styles.listCard}>
          {data.length === 0 ? (
            <Text style={styles.chartEmpty}>Нет данных за период</Text>
          ) : (
            data.slice().reverse().map((d) => (
              <View key={d.date} style={styles.dayRow}>
                <View style={styles.dayDot} />
                <Text style={styles.dayDate}>{formatShortDate(d.date)}</Text>
                <Text style={styles.dayValue}>
                  {metric === 'volume' ? `${formatVolume(d.volume)} ${units}` : formatDuration(d.duration)}
                </Text>
                <Text style={styles.dayCount}>{d.count} трен.</Text>
              </View>
            ))
          )}
        </View>

        <View style={{ height: 40 }} />
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, backgroundColor: '#160808' },
  header: { paddingHorizontal: 16, paddingTop: 8, paddingBottom: 16 },
  badge: {
    flexDirection: 'row', alignItems: 'center', gap: 5,
    backgroundColor: '#EF4444' + '18', paddingHorizontal: 10, paddingVertical: 4,
    borderRadius: 6, alignSelf: 'flex-start', marginBottom: 8,
  },
  badgeText: { color: '#EF4444', fontSize: 11, fontWeight: '800', letterSpacing: 1.5 },
  mainTitle: { color: '#fff', fontSize: 28, fontWeight: '800' },

  // Period
  periodRow: {
    flexDirection: 'row', gap: 6, paddingHorizontal: 16, marginBottom: 12,
  },
  periodBtn: {
    paddingHorizontal: 16, paddingVertical: 8, borderRadius: 8,
    backgroundColor: '#1f0e0e',
  },
  periodBtnActive: { backgroundColor: '#EF4444' },
  periodBtnText: { color: '#666', fontSize: 14, fontWeight: '600' },
  periodBtnTextActive: { color: '#fff' },

  // Metric
  metricRow: {
    flexDirection: 'row', gap: 6, paddingHorizontal: 16, marginBottom: 16,
  },
  metricBtn: {
    flexDirection: 'row', alignItems: 'center', gap: 5,
    paddingHorizontal: 14, paddingVertical: 7, borderRadius: 8,
    backgroundColor: '#1f0e0e',
  },
  metricBtnActive: { backgroundColor: '#EF4444' + '25' },
  metricBtnText: { color: '#666', fontSize: 13, fontWeight: '600' },
  metricBtnTextActive: { color: '#EF4444' },

  // Summary
  summaryRow: { flexDirection: 'row', paddingHorizontal: 16, gap: 8, marginBottom: 16 },
  summaryCard: {
    flex: 1, backgroundColor: '#1f0e0e', borderRadius: 12, padding: 14,
    alignItems: 'center',
  },
  summaryNumber: { color: '#EF4444', fontSize: 18, fontWeight: '800' },
  summaryLabel: { color: '#EF444450', fontSize: 11, marginTop: 4 },

  // Chart
  chartCard: {
    marginHorizontal: 16, marginBottom: 16, backgroundColor: '#1f0e0e',
    borderRadius: 12, padding: 16,
  },
  chart: {
    flexDirection: 'row', alignItems: 'flex-end', height: 130, gap: 2,
  },
  barWrap: { flex: 1, alignItems: 'center', justifyContent: 'flex-end', height: '100%' },
  bar: { width: '70%', borderRadius: 3, backgroundColor: '#EF4444', minWidth: 2 },
  barLabel: { color: '#EF444450', fontSize: 9, marginTop: 3 },
  monthLabelsRow: { flexDirection: 'row', marginTop: 6 },
  monthLabel: { flex: 1, textAlign: 'center', color: '#EF444460', fontSize: 10, fontWeight: '500' },
  chartEmpty: { color: '#666', textAlign: 'center', paddingVertical: 40, fontSize: 14, flex: 1 },

  // List
  listCard: {
    marginHorizontal: 16, backgroundColor: '#1f0e0e', borderRadius: 12, padding: 16,
  },
  dayRow: {
    flexDirection: 'row', alignItems: 'center', paddingVertical: 9,
    borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: '#2a1515', gap: 10,
  },
  dayDot: { width: 6, height: 6, borderRadius: 3, backgroundColor: '#EF4444' },
  dayDate: { color: '#999', fontSize: 13, width: 70 },
  dayValue: { color: '#fff', fontSize: 14, fontWeight: '600', flex: 1 },
  dayCount: { color: '#666', fontSize: 12 },
});
