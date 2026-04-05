import React, { useState, useRef, useMemo } from "react";

import { View, StyleSheet, Text, Pressable, PanResponder } from "react-native";

import {
  format,
  subDays,
  addDays,
  subMonths,
  addMonths,
  subYears,
  addYears,
  startOfMonth,
  endOfMonth,
  eachDayOfInterval,
  startOfWeek,
  endOfWeek,
} from "date-fns";

import { ru } from "date-fns/locale";

import { Ionicons } from "@expo/vector-icons";

import { colors, fontSize, fonts, spacing } from "@/theme";

import { SegmentedControl } from "./SegmentedControl";

const BAR_HEIGHT = 140;

const CHART_TOTAL_HEIGHT = BAR_HEIGHT + 14 + 4 + 18;

const SWIPE_THRESHOLD = 40;

export type Period = "week" | "month" | "year";

interface DayData {
  date: string;

  value: number;
}

interface PeriodChartProps {
  dataByDate: Record<string, number>;

  goal: number;

  color: string;

  formatValue?: (v: number) => string;

  onSelectDate?: (date: string | null) => void;
}

const PERIODS: Period[] = ["week", "month", "year"];

const PERIOD_LABELS: Record<Period, string> = {
  week: "Неделя",

  month: "Месяц",

  year: "Год",
};

// ── data helpers ──

function getAnchorDate(period: Period, offset: number): Date {
  const now = new Date();

  if (period === "week") return offset === 0 ? now : addDays(now, offset * 7);

  if (period === "month") return offset === 0 ? now : addMonths(now, offset);

  return offset === 0 ? now : addYears(now, offset);
}

function getWeekData(
  dataByDate: Record<string, number>,
  offset: number,
): DayData[] {
  const anchor = getAnchorDate("week", offset);

  const days: DayData[] = [];

  // If current week (offset=0), end at today; otherwise show full Mon-Sun

  if (offset === 0) {
    for (let i = 6; i >= 0; i--) {
      const date = format(subDays(anchor, i), "yyyy-MM-dd");

      days.push({ date, value: dataByDate[date] ?? 0 });
    }
  } else {
    const weekStart = startOfWeek(anchor, { weekStartsOn: 1 });

    const weekEnd = endOfWeek(anchor, { weekStartsOn: 1 });

    const allDays = eachDayOfInterval({ start: weekStart, end: weekEnd });

    for (const d of allDays) {
      const date = format(d, "yyyy-MM-dd");

      days.push({ date, value: dataByDate[date] ?? 0 });
    }
  }

  return days;
}

function getMonthData(
  dataByDate: Record<string, number>,
  offset: number,
): DayData[] {
  const anchor = getAnchorDate("month", offset);

  const start = startOfMonth(anchor);

  const end = offset === 0 ? new Date() : endOfMonth(anchor);

  const allDays = eachDayOfInterval({ start, end });

  return allDays.map((d) => {
    const date = format(d, "yyyy-MM-dd");

    return { date, value: dataByDate[date] ?? 0 };
  });
}

function getYearData(
  dataByDate: Record<string, number>,
  offset: number,
): DayData[] {
  const anchor = getAnchorDate("year", offset);

  const months: DayData[] = [];

  for (let i = 11; i >= 0; i--) {
    const monthDate = subMonths(anchor, i);

    const start = startOfMonth(monthDate);

    const isCurrentMonth =
      format(monthDate, "yyyy-MM") === format(new Date(), "yyyy-MM");

    const end = isCurrentMonth ? new Date() : endOfMonth(monthDate);

    const allDays = eachDayOfInterval({ start, end });

    let sum = 0;

    let count = 0;

    for (const d of allDays) {
      const val = dataByDate[format(d, "yyyy-MM-dd")];

      if (val !== undefined && val > 0) {
        sum += val;
        count++;
      }
    }

    months.push({
      date: format(monthDate, "yyyy-MM"),
      value: count > 0 ? Math.round(sum / count) : 0,
    });
  }

  return months;
}

function getBarLabel(date: string, period: Period): string {
  if (period === "year") {
    const [y, m] = date.split("-");

    return format(new Date(Number(y), Number(m) - 1, 1), "LLL", {
      locale: ru,
    }).slice(0, 3);
  }

  const d = new Date(date + "T12:00:00");

  return period === "week"
    ? format(d, "EEEEEE", { locale: ru }).toUpperCase()
    : format(d, "d");
}

function isCurrent(date: string, period: Period): boolean {
  if (period === "year") return date === format(new Date(), "yyyy-MM");

  return date === format(new Date(), "yyyy-MM-dd");
}

function getPeriodLabel(period: Period, offset: number): string {
  const anchor = getAnchorDate(period, offset);

  if (period === "week") {
    if (offset === 0) return "Эта неделя";

    const start =
      offset === 0
        ? subDays(new Date(), 6)
        : startOfWeek(anchor, { weekStartsOn: 1 });

    const end =
      offset === 0 ? new Date() : endOfWeek(anchor, { weekStartsOn: 1 });

    return `${format(start, "d MMM", { locale: ru })} – ${format(end, "d MMM", { locale: ru })}`;
  }

  if (period === "month") {
    if (offset === 0) return format(anchor, "LLLL yyyy", { locale: ru });

    return format(anchor, "LLLL yyyy", { locale: ru });
  }

  // year

  return format(anchor, "yyyy");
}

// ── component ──

export function WeekChart({
  dataByDate,
  goal,
  color,
  formatValue = (v) => String(Math.round(v)),
  onSelectDate,
}: PeriodChartProps) {
  const [period, setPeriod] = useState<Period>("week");

  const [offset, setOffset] = useState(0);

  const [selectedIdx, setSelectedIdx] = useState<number | null>(null);

  // Swipe gesture — use ref for offset to avoid stale closure

  const offsetRef = useRef(offset);

  offsetRef.current = offset;

  const panResponder = useRef(
    PanResponder.create({
      onMoveShouldSetPanResponder: (_, gs) =>
        Math.abs(gs.dx) > 15 && Math.abs(gs.dx) > Math.abs(gs.dy * 1.5),

      onPanResponderRelease: (_, gs) => {
        if (gs.dx > SWIPE_THRESHOLD) {
          setOffset((o) => o - 1);

          setSelectedIdx(null);

          onSelectDate?.(null);
        } else if (gs.dx < -SWIPE_THRESHOLD && offsetRef.current < 0) {
          setOffset((o) => o + 1);

          setSelectedIdx(null);

          onSelectDate?.(null);
        }
      },
    }),
  ).current;

  // Memoize expensive data computation

  const data = useMemo(
    () =>
      period === "week"
        ? getWeekData(dataByDate, offset)
        : period === "month"
          ? getMonthData(dataByDate, offset)
          : getYearData(dataByDate, offset),

    [period, offset, dataByDate],
  );

  const max = useMemo(
    () => Math.max(...data.map((d) => d.value), goal) || 1,
    [data, goal],
  );

  const barWidth =
    period === "week"
      ? 28
      : period === "year"
        ? 18
        : Math.max(Math.floor((300 - data.length * 2) / data.length), 4);

  const showAllLabels = period !== "month";

  const handleBarPress = (index: number) => {
    const newIdx = selectedIdx === index ? null : index;

    setSelectedIdx(newIdx);

    onSelectDate?.(newIdx !== null ? data[newIdx].date : null);
  };

  const handlePeriodChange = (p: Period) => {
    setPeriod(p);

    setOffset(0);

    setSelectedIdx(null);

    onSelectDate?.(null);
  };

  const goBack = () => {
    setOffset((o) => o - 1);

    setSelectedIdx(null);

    onSelectDate?.(null);
  };

  const goForward = () => {
    if (offset >= 0) return;

    setOffset((o) => o + 1);

    setSelectedIdx(null);

    onSelectDate?.(null);
  };

  const goalLineBottom = (goal / max) * BAR_HEIGHT + 18;

  const periodLabel = getPeriodLabel(period, offset);

  return (
    <View>
      {/* Period tabs */}
      <SegmentedControl
        segments={PERIODS.map((p) => PERIOD_LABELS[p])}
        activeIndex={PERIODS.indexOf(period)}
        onChange={(i) => handlePeriodChange(PERIODS[i])}
      />

      {/* Period navigation */}
      <View style={styles.navRow}>
        <Pressable onPress={goBack} hitSlop={12}>
          <Ionicons
            name="chevron-back"
            size={18}
            color={colors.text.secondary}
          />
        </Pressable>
        <Text style={styles.navLabel}>{periodLabel}</Text>
        <Pressable
          onPress={goForward}
          hitSlop={12}
          style={{ opacity: offset < 0 ? 1 : 0.2 }}
        >
          <Ionicons
            name="chevron-forward"
            size={18}
            color={colors.text.secondary}
          />
        </Pressable>
      </View>

      {/* Chart — fixed height, swipeable */}
      <View style={styles.chartContainer} {...panResponder.panHandlers}>
        {/* Goal dashed line */}
        <View style={[styles.goalLine, { bottom: goalLineBottom }]}>
          <View style={styles.goalDash} />
        </View>

        <View style={styles.bars}>
          {data.map((day, i) => {
            const current = isCurrent(day.date, period);

            const selected = selectedIdx === i;

            const highlighted = current || selected;

            const height =
              day.value > 0 ? Math.max((day.value / max) * BAR_HEIGHT, 4) : 2;

            const label = getBarLabel(day.date, period);

            const showLabel =
              showAllLabels || i === 0 || i === data.length - 1 || i % 5 === 0;

            return (
              <Pressable
                key={day.date}
                style={styles.barCol}
                onPress={() => handleBarPress(i)}
              >
                <View style={styles.valueLabelWrap}>
                  {selected && day.value > 0 ? (
                    <Text
                      style={[styles.valueLabel, styles.valueLabelActive]}
                      numberOfLines={1}
                    >
                      {formatValue(day.value)}
                    </Text>
                  ) : null}
                </View>

                <View style={styles.barTrack}>
                  <View
                    style={{
                      height,

                      width: barWidth,

                      backgroundColor: highlighted
                        ? colors.text.primary
                        : color,

                      opacity: highlighted ? 1 : 0.35,

                      borderRadius: Math.min(barWidth / 2, 8),
                    }}
                  />
                </View>

                <View style={styles.dayLabelWrap}>
                  {showLabel ? (
                    <Text
                      style={[
                        styles.dayLabel,
                        highlighted && styles.dayLabelActive,
                      ]}
                      numberOfLines={1}
                    >
                      {label}
                    </Text>
                  ) : null}
                </View>
              </Pressable>
            );
          })}
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  // Navigation

  navRow: {
    flexDirection: "row",

    justifyContent: "space-between",

    alignItems: "center",

    marginBottom: spacing.md,

    paddingHorizontal: spacing.xs,
  },

  navLabel: {
    fontSize: fontSize.sm,

    fontFamily: fonts.regular,

    color: colors.text.secondary,

    textTransform: "capitalize",
  },

  // Chart

  chartContainer: {
    height: CHART_TOTAL_HEIGHT,

    position: "relative",
  },

  goalLine: {
    position: "absolute",

    left: 0,

    right: 0,

    height: 1,

    zIndex: 1,
  },

  goalDash: {
    height: 1,

    borderStyle: "dashed",

    borderWidth: 1,

    borderColor: "rgba(255,255,255,0.1)",
  },

  bars: {
    flex: 1,

    flexDirection: "row",

    justifyContent: "space-between",

    alignItems: "flex-end",
  },

  barCol: {
    alignItems: "center",

    flex: 1,

    height: CHART_TOTAL_HEIGHT,

    justifyContent: "flex-end",
  },

  valueLabelWrap: {
    height: 14,

    justifyContent: "center",

    marginBottom: 4,
  },

  valueLabel: {
    fontSize: 9,

    fontFamily: fonts.monoBold,

    color: colors.text.muted,
  },

  valueLabelActive: {
    color: colors.text.primary,
  },

  barTrack: {
    height: BAR_HEIGHT,

    justifyContent: "flex-end",

    alignItems: "center",
  },

  dayLabelWrap: {
    height: 18,

    justifyContent: "center",

    marginTop: 6,
  },

  dayLabel: {
    fontSize: 9,

    fontFamily: fonts.semiBold,

    color: colors.text.muted,

    textTransform: "capitalize",
  },

  dayLabelActive: {
    color: colors.text.primary,
  },
});
