import React from "react";

import { View, StyleSheet, Text, Pressable } from "react-native";

import Svg, { Circle } from "react-native-svg";

import { Ionicons } from "@expo/vector-icons";

import { colors, spacing, fonts, fontSize } from "@/theme";

import { Card } from "./Card";

interface ArcHeroProps {
  current: number;

  goal: number;

  unit: string;

  color: string;

  onPress?: () => void;
}

const ARC_SIZE = 160;

const STROKE_W = 8;

const R = (ARC_SIZE - STROKE_W) / 2;

const CIRCUMFERENCE = 2 * Math.PI * R;

const ARC_FRACTION = 0.75;

const ARC_LENGTH = CIRCUMFERENCE * ARC_FRACTION;

const ARC_GAP = CIRCUMFERENCE * (1 - ARC_FRACTION);

const ROTATION = 135;

export function ArcHero({ current, goal, unit, color, onPress }: ArcHeroProps) {
  const remaining = goal - current;

  const isOver = remaining < 0;

  const progress = goal > 0 ? Math.min(current / goal, 1) : 0;

  const filledLength = progress * ARC_LENGTH;

  const emptyLength = ARC_LENGTH - filledLength;

  const displayValue = isOver
    ? `−${Math.abs(Math.round(remaining))}`
    : `${Math.round(remaining)}`;

  return (
    <Pressable onPress={onPress} disabled={!onPress}>
      <Card style={styles.inner}>
        {onPress && (
          <View style={styles.chevronWrap}>
            <Ionicons
              name="chevron-forward"
              size={18}
              color={colors.text.muted}
            />
          </View>
        )}

        <View style={styles.arcWrap}>
          <Svg width={ARC_SIZE} height={ARC_SIZE}>
            <Circle
              cx={ARC_SIZE / 2}
              cy={ARC_SIZE / 2}
              r={R}
              stroke={colors.trackBg}
              strokeWidth={STROKE_W}
              fill="none"
              strokeDasharray={`${ARC_LENGTH} ${ARC_GAP}`}
              strokeLinecap="round"
              rotation={ROTATION}
              origin={`${ARC_SIZE / 2}, ${ARC_SIZE / 2}`}
            />
            <Circle
              cx={ARC_SIZE / 2}
              cy={ARC_SIZE / 2}
              r={R}
              stroke={isOver ? colors.over : color}
              strokeWidth={STROKE_W}
              fill="none"
              strokeDasharray={`${filledLength} ${emptyLength + ARC_GAP}`}
              strokeLinecap="round"
              rotation={ROTATION}
              origin={`${ARC_SIZE / 2}, ${ARC_SIZE / 2}`}
            />
          </Svg>
          <View style={styles.arcCenter}>
            <Text style={[styles.value, isOver && styles.valueOver]}>
              {displayValue}
            </Text>
            <Text style={styles.label}>{unit} ОСТАЛОСЬ</Text>
          </View>
        </View>
      </Card>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  inner: {
    alignItems: "center",
  },

  chevronWrap: {
    position: "absolute",

    top: spacing.lg,

    right: spacing.lg,
  },

  arcWrap: {
    width: ARC_SIZE,

    height: ARC_SIZE,
  },

  arcCenter: {
    ...StyleSheet.absoluteFillObject,

    justifyContent: "center",

    alignItems: "center",
  },

  value: {
    fontFamily: fonts.display,

    fontSize: 40,

    color: colors.text.primary,
  },

  valueOver: {
    color: colors.over,
  },

  label: {
    fontSize: 10,

    fontFamily: fonts.semiBold,

    color: colors.text.muted,

    letterSpacing: 1,

    marginTop: 2,
  },
});
