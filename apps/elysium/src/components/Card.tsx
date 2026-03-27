import React from 'react';
import { View, StyleSheet, ViewStyle } from 'react-native';
import { colors, spacing, cardRadius } from '@/theme';

interface CardProps {
  children: React.ReactNode;
  noPadding?: boolean;
  style?: ViewStyle;
}

export function Card({ children, noPadding, style }: CardProps) {
  return (
    <View style={[styles.card, !noPadding && styles.padded, style]}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.bg.card,
    borderRadius: cardRadius,
    overflow: 'hidden',
  },
  padded: {
    padding: spacing.lg,
  },
});
