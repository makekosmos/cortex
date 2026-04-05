import React, { useEffect, useRef, useCallback } from "react";

import {
  View,
  StyleSheet,
  Text,
  Pressable,
  Animated,
  LayoutChangeEvent,
} from "react-native";

import { colors, fontSize, fonts, cardRadius } from "@/theme";

interface SegmentedControlProps {
  segments: string[];

  activeIndex: number;

  onChange: (index: number) => void;
}

const PAD = 4;

const H = 36;

export function SegmentedControl({
  segments,
  activeIndex,
  onChange,
}: SegmentedControlProps) {
  const [widths, setWidths] = React.useState<number[]>(segments.map(() => 0));

  const translateX = useRef(new Animated.Value(0)).current;

  const indicatorWidth = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    const x = widths.slice(0, activeIndex).reduce((a, w) => a + w, 0);

    Animated.spring(translateX, {
      toValue: x,

      damping: 18,

      stiffness: 200,

      mass: 0.8,

      useNativeDriver: false,
    }).start();

    Animated.spring(indicatorWidth, {
      toValue: widths[activeIndex] || 0,

      damping: 18,

      stiffness: 200,

      mass: 0.8,

      useNativeDriver: false,
    }).start();
  }, [activeIndex, widths]);

  const onLayout = useCallback(
    (index: number) => (e: LayoutChangeEvent) => {
      const w = e.nativeEvent.layout.width;

      setWidths((prev) => {
        const next = [...prev];

        next[index] = w;

        return next;
      });
    },

    [],
  );

  return (
    <View style={styles.container}>
      <Animated.View
        style={[
          styles.indicator,

          { transform: [{ translateX }], width: indicatorWidth },
        ]}
      />
      {segments.map((label, i) => (
        <Pressable
          key={label}
          style={styles.segment}
          onLayout={onLayout(i)}
          onPress={() => onChange(i)}
        >
          <Text style={[styles.label, activeIndex === i && styles.labelActive]}>
            {label}
          </Text>
        </Pressable>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",

    backgroundColor: colors.bg.card,

    borderRadius: cardRadius,

    padding: PAD,

    height: H + PAD * 2,
  },

  segment: {
    flex: 1,

    justifyContent: "center",

    alignItems: "center",

    height: H,

    borderRadius: cardRadius - PAD,
  },

  label: {
    fontSize: fontSize.sm,

    fontFamily: fonts.semiBold,

    color: colors.text.muted,
  },

  labelActive: {
    color: colors.text.primary,
  },

  indicator: {
    position: "absolute",

    left: PAD,

    top: PAD,

    height: H,

    borderRadius: cardRadius - PAD,

    backgroundColor: colors.bg.elevated,

    borderWidth: 1,

    borderColor: "rgba(255,255,255,0.12)",
  },
});
