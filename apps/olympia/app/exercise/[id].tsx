import { useState, useEffect } from "react";

import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  Image,
  Dimensions,
} from "react-native";

import { useLocalSearchParams, Stack } from "expo-router";

import { useThemeColor } from "@/lib/useThemeColor";

import { getExerciseById, getExerciseHistory } from "@/lib/database";

import { useSettingsStore } from "@/lib/stores/settings-store";

import { musclesRu } from "@/lib/types";

const IMAGE_BASE =
  "https://raw.githubusercontent.com/yuhonas/free-exercise-db/main/exercises/";

const { width: SCREEN_WIDTH } = Dimensions.get("window");

export default function ExerciseDetailScreen() {
  const colors = useThemeColor();

  const units = useSettingsStore((s) => s.units);

  const { id } = useLocalSearchParams<{ id: string }>();

  const [exercise, setExercise] = useState<any>(null);

  const [history, setHistory] = useState<any[]>([]);

  useEffect(() => {
    if (id) {
      Promise.all([getExerciseById(id), getExerciseHistory(id)]).then(
        ([ex, hist]) => {
          setExercise(ex);

          setHistory(hist);
        },
      );
    }
  }, [id]);

  if (!exercise)
    return (
      <View
        style={[styles.container, { backgroundColor: colors.background }]}
      />
    );

  const prsByDate = new Map<
    string,
    { maxWeight: number; max1RM: number; totalVolume: number }
  >();

  for (const h of history) {
    const date = h.started_at.split("T")[0];

    const weight = h.weight_kg || 0;

    const reps = h.reps || 0;

    const e1rm = weight * (1 + reps / 30);

    const volume = weight * reps;

    if (!prsByDate.has(date)) {
      prsByDate.set(date, {
        maxWeight: weight,
        max1RM: e1rm,
        totalVolume: volume,
      });
    } else {
      const existing = prsByDate.get(date)!;

      existing.maxWeight = Math.max(existing.maxWeight, weight);

      existing.max1RM = Math.max(existing.max1RM, e1rm);

      existing.totalVolume += volume;
    }
  }

  const chartData = Array.from(prsByDate.entries()).map(([date, d]) => ({
    date,
    ...d,
  }));

  const allTimeMax = chartData.reduce(
    (max, d) => Math.max(max, d.maxWeight),
    0,
  );

  const allTime1RM = chartData.reduce((max, d) => Math.max(max, d.max1RM), 0);

  const maxChartVal = Math.max(...chartData.map((d) => d.max1RM), 1);

  function displayWeight(kg: number) {
    if (units === "lbs") return `${(kg * 2.205).toFixed(0)} lbs`;

    return `${kg.toFixed(1)} кг`;
  }

  return (
    <>
      <Stack.Screen options={{ title: exercise.name }} />
      <ScrollView
        style={[styles.container, { backgroundColor: colors.background }]}
      >
        {exercise.images.length > 0 && (
          <ScrollView
            horizontal
            pagingEnabled
            showsHorizontalScrollIndicator={false}
          >
            {exercise.images.map((img: string, i: number) => (
              <Image
                key={i}
                source={{ uri: IMAGE_BASE + img }}
                style={styles.image}
                resizeMode="contain"
              />
            ))}
          </ScrollView>
        )}

        <View style={styles.infoSection}>
          <View style={styles.badges}>
            {exercise.level && (
              <View style={[styles.badge, { backgroundColor: colors.surface }]}>
                <Text
                  style={[styles.badgeText, { color: colors.textSecondary }]}
                >
                  {exercise.level}
                </Text>
              </View>
            )}
            {exercise.equipment && (
              <View style={[styles.badge, { backgroundColor: colors.surface }]}>
                <Text
                  style={[styles.badgeText, { color: colors.textSecondary }]}
                >
                  {exercise.equipment}
                </Text>
              </View>
            )}
            {exercise.force && (
              <View style={[styles.badge, { backgroundColor: colors.surface }]}>
                <Text
                  style={[styles.badgeText, { color: colors.textSecondary }]}
                >
                  {exercise.force}
                </Text>
              </View>
            )}
            {exercise.mechanic && (
              <View style={[styles.badge, { backgroundColor: colors.surface }]}>
                <Text
                  style={[styles.badgeText, { color: colors.textSecondary }]}
                >
                  {exercise.mechanic}
                </Text>
              </View>
            )}
          </View>

          <Text style={[styles.label, { color: colors.textTertiary }]}>
            Основные мышцы
          </Text>
          <Text style={[styles.value, { color: colors.text }]}>
            {musclesRu(exercise.primary_muscles)}
          </Text>

          {exercise.secondary_muscles.length > 0 && (
            <>
              <Text style={[styles.label, { color: colors.textTertiary }]}>
                Вспомогательные
              </Text>
              <Text style={[styles.value, { color: colors.text }]}>
                {musclesRu(exercise.secondary_muscles)}
              </Text>
            </>
          )}
        </View>

        {chartData.length > 0 && (
          <View style={[styles.section, { backgroundColor: colors.surface }]}>
            <Text style={[styles.sectionTitle, { color: colors.text }]}>
              Личные рекорды
            </Text>
            <View style={styles.prRow}>
              <View style={styles.prItem}>
                <Text style={[styles.prValue, { color: colors.accent }]}>
                  {displayWeight(allTimeMax)}
                </Text>
                <Text style={[styles.prLabel, { color: colors.textSecondary }]}>
                  Макс. вес
                </Text>
              </View>
              <View style={styles.prItem}>
                <Text style={[styles.prValue, { color: colors.accent }]}>
                  {displayWeight(allTime1RM)}
                </Text>
                <Text style={[styles.prLabel, { color: colors.textSecondary }]}>
                  Расч. 1ПМ
                </Text>
              </View>
            </View>
          </View>
        )}

        {chartData.length > 1 && (
          <View style={[styles.section, { backgroundColor: colors.surface }]}>
            <Text style={[styles.sectionTitle, { color: colors.text }]}>
              Прогресс (расч. 1ПМ)
            </Text>
            <View style={styles.chart}>
              {chartData.slice(-10).map((d, i) => {
                const heightPercent = (d.max1RM / maxChartVal) * 100;

                return (
                  <View key={i} style={styles.chartBar}>
                    <View
                      style={[
                        styles.bar,
                        {
                          height: `${heightPercent}%`,
                          backgroundColor: colors.accent,
                        },
                      ]}
                    />
                    <Text
                      style={[
                        styles.chartLabel,
                        { color: colors.textTertiary },
                      ]}
                    >
                      {d.date.slice(5)}
                    </Text>
                  </View>
                );
              })}
            </View>
          </View>
        )}

        {exercise.instructions.length > 0 && (
          <View style={[styles.section, { backgroundColor: colors.surface }]}>
            <Text style={[styles.sectionTitle, { color: colors.text }]}>
              Инструкция
            </Text>
            {exercise.instructions.map((inst: string, i: number) => (
              <Text
                key={i}
                style={[styles.instruction, { color: colors.textSecondary }]}
              >
                {i + 1}. {inst}
              </Text>
            ))}
          </View>
        )}

        <View style={{ height: 40 }} />
      </ScrollView>
    </>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },

  image: { width: SCREEN_WIDTH, height: 250 },

  infoSection: { padding: 16 },

  badges: { flexDirection: "row", flexWrap: "wrap", gap: 8, marginBottom: 16 },

  badge: { paddingHorizontal: 12, paddingVertical: 4, borderRadius: 12 },

  badgeText: { fontSize: 13, fontWeight: "500", textTransform: "capitalize" },

  label: {
    fontSize: 12,
    fontWeight: "600",
    marginTop: 12,
    textTransform: "uppercase",
  },

  value: { fontSize: 15, marginTop: 4, textTransform: "capitalize" },

  section: { margin: 16, marginTop: 0, padding: 16, borderRadius: 12 },

  sectionTitle: { fontSize: 17, fontWeight: "700", marginBottom: 12 },

  prRow: { flexDirection: "row", gap: 16 },

  prItem: { flex: 1, alignItems: "center" },

  prValue: { fontSize: 22, fontWeight: "700" },

  prLabel: { fontSize: 12, marginTop: 4 },

  chart: { flexDirection: "row", alignItems: "flex-end", height: 120, gap: 4 },

  chartBar: {
    flex: 1,
    alignItems: "center",
    height: "100%",
    justifyContent: "flex-end",
  },

  bar: { width: "80%", borderRadius: 4, minHeight: 4 },

  chartLabel: { fontSize: 9, marginTop: 4 },

  instruction: { fontSize: 14, lineHeight: 20, marginBottom: 8 },
});
