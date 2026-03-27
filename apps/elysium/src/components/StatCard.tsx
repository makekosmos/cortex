import React from 'react';
import { View, StyleSheet, Text } from 'react-native';
import { colors, fontSize, fonts, spacing, cardRadius } from '@/theme';

interface StatCardProps {
  value: string;
  unit: string;
  label: string;
  status?: string;
  statusColor?: string;
}

export function StatCard({ value, unit, label, status, statusColor = colors.accent }: StatCardProps) {
  return (
    <View style={styles.card}>
      <Text style={styles.value}>
        {value}
        <Text style={styles.unit}>{unit}</Text>
      </Text>
      <Text style={styles.label}>{label}</Text>
      {status && <Text style={[styles.status, { color: statusColor }]}>{status}</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    flex: 1,
    backgroundColor: colors.bg.card,
    borderRadius: cardRadius,
    padding: spacing.lg,
  },
  value: {
    fontSize: fontSize.xxl,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
  },
  unit: {
    fontSize: fontSize.sm,
    fontFamily: fonts.regular,
    color: colors.text.muted,
  },
  label: {
    fontSize: fontSize.xs,
    fontFamily: fonts.semiBold,
    color: colors.text.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    marginTop: 6,
  },
  status: {
    fontSize: fontSize.sm,
    fontFamily: fonts.semiBold,
    marginTop: 4,
  },
});
