import React, { useState } from 'react';
import { View, ScrollView, StyleSheet, Text, TextInput, Pressable } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, fonts } from '@/theme';
import { Card } from '@/components/Card';
import { CardRow } from '@/components/CardRow';

const PAD = spacing.lg;

export default function ProfileScreen() {
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const [weight, setWeight] = useState('75');
  const [height, setHeight] = useState('178');

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={[styles.content, { paddingTop: insets.top + 12 }]}
      showsVerticalScrollIndicator={false}
    >
      {/* Header with settings */}
      <View style={styles.topBar}>
        <View style={{ width: 22 }} />
        <Pressable onPress={() => router.push('/settings')} hitSlop={12}>
          <Ionicons name="cog-outline" size={22} color={colors.text.secondary} />
        </Pressable>
      </View>

      {/* Body data */}
      <Card noPadding>
        <CardRow first>
          <Text style={styles.rowLabel}>Вес (кг)</Text>
          <TextInput
            style={styles.input}
            value={weight}
            onChangeText={setWeight}
            keyboardType="decimal-pad"
            textAlign="right"
          />
        </CardRow>
        <CardRow>
          <Text style={styles.rowLabel}>Рост (см)</Text>
          <TextInput
            style={styles.input}
            value={height}
            onChangeText={setHeight}
            keyboardType="number-pad"
            textAlign="right"
          />
        </CardRow>
      </Card>

      <View style={{ height: 100 }} />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: { flex: 1, backgroundColor: colors.bg.primary },
  content: {},
  topBar: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: PAD,
    marginBottom: spacing.lg,
  },
  rowLabel: { fontSize: fontSize.md, fontFamily: fonts.regular, color: colors.text.primary },
  input: {
    width: 80,
    height: 36,
    backgroundColor: colors.bg.input,
    borderRadius: 10,
    paddingHorizontal: spacing.sm,
    fontSize: fontSize.md,
    fontFamily: fonts.monoBold,
    color: colors.text.primary,
  },
});
