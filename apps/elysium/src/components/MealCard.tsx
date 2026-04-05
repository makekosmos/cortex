import React from "react";

import { View, StyleSheet, Text, Pressable } from "react-native";

import { Ionicons } from "@expo/vector-icons";

import { colors, fontSize, spacing, fonts } from "@/theme";

import { Card } from "./Card";

import { CardRow } from "./CardRow";

import type { MealEntry, MealType } from "@/types/nutrition";

import { MEAL_TYPE_LABELS } from "@/types/nutrition";

import { sumMacros, formatCalories } from "@/utils/macros";

const MEAL_ICONS: Record<MealType, keyof typeof Ionicons.glyphMap> = {
  breakfast: "sunny",

  lunch: "restaurant",

  dinner: "moon",

  snack: "cafe",
};

interface MealCardProps {
  mealType: MealType;

  entries: MealEntry[];

  onAdd: (mealType: MealType) => void;

  onEdit: (entry: MealEntry) => void;

  onRemove: (id: string) => void;
}

export const MealCard = React.memo(function MealCard({
  mealType,
  entries,
  onAdd,
  onEdit,
  onRemove,
}: MealCardProps) {
  const totals = sumMacros(entries);

  const iconName = MEAL_ICONS[mealType];

  const accentColor = colors.meal[mealType];

  return (
    <Card noPadding style={styles.card}>
      <Pressable style={styles.header} onPress={() => onAdd(mealType)}>
        <View style={styles.headerLeft}>
          <View
            style={[styles.iconCircle, { backgroundColor: accentColor + "18" }]}
          >
            <Ionicons name={iconName} size={16} color={accentColor} />
          </View>
          <View style={{ flex: 1 }}>
            <Text style={styles.title}>{MEAL_TYPE_LABELS[mealType]}</Text>
            {entries.length > 0 && (
              <Text style={styles.subtitle}>
                {formatCalories(totals.calories)} ккал
              </Text>
            )}
          </View>
        </View>
        <Ionicons name="add-circle" size={22} color={colors.text.muted} />
      </Pressable>

      {entries.map((entry, index) => (
        <CardRow
          key={entry.id}
          first={index === 0 && false}
          onPress={() => onEdit(entry)}
          onLongPress={() => onRemove(entry.id)}
        >
          <View style={styles.entryInfo}>
            <Text style={styles.entryName} numberOfLines={1}>
              {entry.foodItem.name}
            </Text>
            <Text style={styles.entryDetail}>
              {entry.quantity} x {entry.foodItem.servingSize}
              {entry.foodItem.servingUnit}
            </Text>
          </View>
          <Text style={styles.entryCal}>
            {formatCalories(entry.foodItem.macros.calories * entry.quantity)}
          </Text>
        </CardRow>
      ))}

      {entries.length === 0 && (
        <CardRow onPress={() => onAdd(mealType)} style={styles.emptyRow}>
          <Text style={styles.emptyText}>Добавить</Text>
        </CardRow>
      )}
    </Card>
  );
});

const styles = StyleSheet.create({
  card: {
    marginBottom: spacing.xs,
  },

  header: {
    flexDirection: "row",

    justifyContent: "space-between",

    alignItems: "center",

    padding: spacing.lg,
  },

  headerLeft: {
    flexDirection: "row",

    alignItems: "center",

    gap: spacing.md,

    flex: 1,
  },

  iconCircle: {
    width: 36,

    height: 36,

    borderRadius: 12,

    justifyContent: "center",

    alignItems: "center",
  },

  title: {
    fontSize: fontSize.md,

    fontFamily: fonts.semiBold,

    color: colors.text.primary,
  },

  subtitle: {
    fontSize: fontSize.xs,

    fontFamily: fonts.mono,

    color: colors.text.muted,

    marginTop: 2,
  },

  entryInfo: { flex: 1, marginRight: spacing.md },

  entryName: {
    fontSize: fontSize.md,
    fontFamily: fonts.regular,
    color: colors.text.primary,
  },

  entryDetail: {
    fontSize: fontSize.xs,
    fontFamily: fonts.mono,
    color: colors.text.muted,
    marginTop: 2,
  },

  entryCal: {
    fontSize: fontSize.md,
    fontFamily: fonts.monoBold,
    color: colors.text.secondary,
  },

  emptyRow: {
    justifyContent: "center",
  },

  emptyText: {
    fontSize: fontSize.sm,
    fontFamily: fonts.regular,
    color: colors.text.muted,
  },
});
