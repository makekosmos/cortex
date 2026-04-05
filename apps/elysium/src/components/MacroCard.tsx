import React, { useMemo } from "react";

import { View, StyleSheet, Text } from "react-native";

import { colors, fontSize, fonts, spacing } from "@/theme";

import { Card } from "./Card";

interface MacroData {
  label: string;

  current: number;

  goal: number;

  color: string;
}

interface MacroCardProps {
  macros: MacroData[];
}

const COLS = 9;

const ROWS = 4;

const TOTAL_CELLS = COLS * ROWS;

const CELL_SIZE = 9;

const CELL_GAP = 3;

const COLOR_EMPTY = colors.trackBg;

const COLOR_OVER = colors.over;

function MacroColumn({ data }: { data: MacroData }) {
  const { current, goal, color } = data;

  const isOver = current > goal;

  const filledCount = useMemo(() => {
    if (current <= 0 || goal <= 0) return 0;

    if (isOver) return Math.round((goal / current) * TOTAL_CELLS);

    return Math.min(Math.round((current / goal) * TOTAL_CELLS), TOTAL_CELLS);
  }, [current, goal, isOver]);

  const overCount = useMemo(() => {
    if (!isOver || current <= 0) return 0;

    return TOTAL_CELLS - filledCount;
  }, [isOver, current, filledCount]);

  const cells = useMemo(() => {
    const arr: string[] = [];

    for (let i = 0; i < TOTAL_CELLS; i++) {
      if (i < filledCount) arr.push(color);
      else if (i < filledCount + overCount) arr.push(COLOR_OVER);
      else arr.push(COLOR_EMPTY);
    }

    return arr;
  }, [filledCount, overCount, color]);

  return (
    <View style={colStyles.container}>
      <View style={colStyles.grid}>
        {cells.map((c, i) => (
          <View key={i} style={[colStyles.cell, { backgroundColor: c }]} />
        ))}
      </View>
      <View style={colStyles.infoRow}>
        <Text style={[colStyles.label, { color }]}>{data.label}</Text>
        <Text
          style={[
            colStyles.label,
            isOver ? { color: COLOR_OVER } : { color: colors.text.secondary },
          ]}
        >
          {Math.round(current)}г
        </Text>
      </View>
    </View>
  );
}

const gridWidth = COLS * CELL_SIZE + (COLS - 1) * CELL_GAP;

const colStyles = StyleSheet.create({
  container: { alignItems: "flex-start" },

  grid: {
    width: gridWidth,

    flexDirection: "row",

    flexWrap: "wrap-reverse",

    gap: CELL_GAP,
  },

  cell: {
    width: CELL_SIZE,

    height: CELL_SIZE,

    borderRadius: 2,
  },

  infoRow: {
    flexDirection: "row",

    alignItems: "center",

    gap: 4,

    marginTop: spacing.sm,
  },

  label: {
    fontSize: fontSize.xs,

    fontFamily: fonts.semiBold,
  },
});

export function MacroCard({ macros }: MacroCardProps) {
  return (
    <Card>
      <View style={styles.columnsRow}>
        {macros.map((m) => (
          <MacroColumn key={m.label} data={m} />
        ))}
      </View>
    </Card>
  );
}

const styles = StyleSheet.create({
  columnsRow: {
    flexDirection: "row",

    justifyContent: "space-between",
  },
});
