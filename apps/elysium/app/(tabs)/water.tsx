import React, { useState } from "react";

import {
  View,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  Pressable,
} from "react-native";

import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useRouter } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { format } from "date-fns";

import { colors, fontSize, spacing, cardRadius, fonts } from "@/theme";

import { useWaterStore } from "@/stores/water-store";

import { ArcHero } from "@/components/ArcHero";

import { Card } from "@/components/Card";

import { CardRow } from "@/components/CardRow";

const PRESETS = [150, 250, 300, 500];

export default function WaterScreen() {
  const insets = useSafeAreaInsets();

  const router = useRouter();

  const { goal, addWater, removeWater, getToday, getTodayTotal } =
    useWaterStore();

  const [customMl, setCustomMl] = useState("");

  const total = getTodayTotal();

  const entries = getToday();

  const handleCustomAdd = () => {
    const val = parseInt(customMl, 10);

    if (!isNaN(val) && val > 0) {
      addWater(val);

      setCustomMl("");
    }
  };

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={[styles.content, { paddingTop: insets.top + 12 }]}
      showsVerticalScrollIndicator={false}
    >
      {/* Arc hero — same style as calories */}
      <ArcHero
        current={total}
        goal={goal}
        unit="мл"
        color={colors.water}
        onPress={() => router.push("/water-stats")}
      />

      <View style={{ height: spacing.xs }} />

      {/* Presets + custom input */}
      <Card>
        <View style={styles.presetsRow}>
          {PRESETS.map((ml) => (
            <Pressable
              key={ml}
              style={styles.presetBtn}
              onPress={() => addWater(ml)}
            >
              <Text style={styles.presetText}>{ml}</Text>
            </Pressable>
          ))}
        </View>

        <View style={styles.customRow}>
          <TextInput
            style={styles.customInput}
            placeholder="мл"
            placeholderTextColor={colors.text.muted}
            value={customMl}
            onChangeText={setCustomMl}
            keyboardType="number-pad"
            textAlign="center"
            onSubmitEditing={handleCustomAdd}
          />
          <Pressable style={styles.addBtn} onPress={handleCustomAdd}>
            <Ionicons name="add" size={20} color={colors.text.primary} />
          </Pressable>
        </View>
      </Card>

      {/* History */}
      {entries.length > 0 && (
        <>
          <View style={{ height: spacing.xs }} />
          <Card noPadding>
            {[...entries].reverse().map((entry, index) => (
              <CardRow
                key={entry.id}
                first={index === 0}
                onLongPress={() => removeWater(entry.id)}
              >
                <View style={styles.historyLeft}>
                  <Ionicons name="water" size={16} color={colors.water} />
                  <Text style={styles.historyAmount}>{entry.amount} мл</Text>
                </View>
                <Text style={styles.historyTime}>
                  {format(new Date(entry.time), "HH:mm")}
                </Text>
              </CardRow>
            ))}
          </Card>
        </>
      )}

      <View style={{ height: 100 }} />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: { flex: 1, backgroundColor: colors.bg.primary },

  content: {},

  presetsRow: {
    flexDirection: "row",

    gap: spacing.sm,

    marginBottom: spacing.md,
  },

  presetBtn: {
    flex: 1,

    backgroundColor: colors.bg.input,

    borderRadius: 10,

    paddingVertical: spacing.sm,

    alignItems: "center",
  },

  presetText: {
    fontSize: fontSize.sm,

    fontFamily: fonts.monoBold,

    color: colors.water,
  },

  customRow: {
    flexDirection: "row",

    gap: spacing.sm,
  },

  customInput: {
    flex: 1,

    height: 40,

    backgroundColor: colors.bg.input,

    borderRadius: 10,

    paddingHorizontal: spacing.md,

    fontSize: fontSize.md,

    fontFamily: fonts.mono,

    color: colors.text.primary,
  },

  addBtn: {
    width: 40,

    height: 40,

    borderRadius: 10,

    backgroundColor: colors.water,

    justifyContent: "center",

    alignItems: "center",
  },

  historyLeft: {
    flexDirection: "row",

    alignItems: "center",

    gap: spacing.sm,
  },

  historyAmount: {
    fontSize: fontSize.md,

    color: colors.text.primary,

    fontFamily: fonts.regular,
  },

  historyTime: {
    fontSize: fontSize.sm,

    fontFamily: fonts.mono,

    color: colors.text.muted,
  },
});
