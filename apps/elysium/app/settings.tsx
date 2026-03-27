import React, { useState, useEffect } from 'react';
import { View, ScrollView, StyleSheet, Text, TextInput, Pressable, Alert } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, cardRadius, fonts } from '@/theme';
import { useNutritionStore } from '@/stores/nutrition-store';
import { useSettingsStore, type FoodSource } from '@/stores/settings-store';
import { useSyncStore } from '@/sync/sync-store';
import { Card } from '@/components/Card';
import { CardRow } from '@/components/CardRow';
import type { Macros } from '@/types/nutrition';
import { caloriesFromMacros } from '@/utils/macros';

const PAD = spacing.lg;

export default function SettingsScreen() {
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const { goals, updateGoals } = useNutritionStore();
  const { foodSource, setFoodSource } = useSettingsStore();
  const sync = useSyncStore();
  const [form, setForm] = useState<Macros>({ ...goals });
  const [syncUrl, setSyncUrl] = useState(sync.serverUrl);
  const [syncKey, setSyncKey] = useState(sync.apiKey);

  useEffect(() => {
    sync.hydrate();
  }, []);

  useEffect(() => {
    setSyncUrl(sync.serverUrl);
    setSyncKey(sync.apiKey);
  }, [sync.serverUrl, sync.apiKey]);

  const computedCalories = caloriesFromMacros(form.protein, form.fat, form.carbs);

  const handleSave = () => { updateGoals(form); Alert.alert('Готово', 'Цели обновлены'); };

  const fields: { key: keyof Macros; label: string; unit: string }[] = [
    { key: 'protein', label: 'Белки', unit: 'г' },
    { key: 'fat', label: 'Жиры', unit: 'г' },
    { key: 'carbs', label: 'Углеводы', unit: 'г' },
  ];

  return (
    <ScrollView style={styles.scroll} contentContainerStyle={[styles.content, { paddingTop: insets.top + 12 }]}>
      <View style={styles.header}>
        <Pressable onPress={() => router.back()} hitSlop={12} style={styles.backBtn}>
          <Ionicons name="chevron-back" size={24} color={colors.accent} />
        </Pressable>
        <Text style={styles.headerTitle}>Настройки</Text>
        <View style={{ width: 36 }} />
      </View>

      <Card noPadding>
        {fields.map(({ key, label, unit }, index) => (
          <CardRow key={key} first={index === 0}>
            <Text style={styles.rowLabel}>{label} ({unit})</Text>
            <TextInput
              style={styles.input}
              value={String(form[key])}
              onChangeText={(text) => {
                const val = parseInt(text, 10);
                if (!isNaN(val)) setForm((p) => ({ ...p, [key]: val }));
                else if (text === '') setForm((p) => ({ ...p, [key]: 0 }));
              }}
              keyboardType="number-pad"
              textAlign="right"
            />
          </CardRow>
        ))}
        <CardRow>
          <Text style={styles.rowLabel}>Калории (ккал)</Text>
          <Text style={styles.computed}>{computedCalories}</Text>
        </CardRow>
      </Card>

      <Pressable style={styles.saveBtn} onPress={handleSave}>
        <Text style={styles.saveBtnText}>Сохранить</Text>
      </Pressable>

      <Text style={styles.sectionLabel}>Источник данных</Text>
      <Card noPadding style={styles.sectionCard}>
        {([
          { key: 'fatsecret' as FoodSource, label: 'FatSecret', desc: 'Русская база, точные данные' },
          { key: 'openfoodfacts' as FoodSource, label: 'Open Food Facts', desc: 'Открытая база, штрих-коды' },
        ]).map(({ key, label, desc }, index) => (
          <CardRow key={key} first={index === 0}>
            <Pressable style={styles.sourceRow} onPress={() => setFoodSource(key)}>
              <View style={styles.sourceInfo}>
                <Text style={styles.rowLabel}>{label}</Text>
                <Text style={styles.sourceDesc}>{desc}</Text>
              </View>
              <Ionicons
                name={foodSource === key ? 'checkmark-circle' : 'ellipse-outline'}
                size={22}
                color={foodSource === key ? colors.accent : colors.text.muted}
              />
            </Pressable>
          </CardRow>
        ))}
      </Card>

      <Text style={styles.sectionLabel}>Синхронизация</Text>
      <Card noPadding style={styles.sectionCard}>
        <CardRow first>
          <View style={styles.syncField}>
            <Text style={styles.rowLabel}>Сервер</Text>
            <TextInput
              style={[styles.input, styles.syncInput]}
              value={syncUrl}
              onChangeText={setSyncUrl}
              placeholder="ws://192.168.1.x:8000"
              placeholderTextColor={colors.text.muted}
              autoCapitalize="none"
              autoCorrect={false}
            />
          </View>
        </CardRow>
        <CardRow>
          <View style={styles.syncField}>
            <Text style={styles.rowLabel}>API Key</Text>
            <TextInput
              style={[styles.input, styles.syncInput]}
              value={syncKey}
              onChangeText={setSyncKey}
              placeholder="xxxxxxxx"
              placeholderTextColor={colors.text.muted}
              autoCapitalize="none"
              autoCorrect={false}
              secureTextEntry
            />
          </View>
        </CardRow>
        <CardRow>
          <View style={styles.syncStatusRow}>
            <View style={styles.syncStatusLeft}>
              <View style={[styles.dot, sync.isConnected ? styles.dotOn : sync.isSyncing ? styles.dotSyncing : styles.dotOff]} />
              <Text style={styles.rowLabel}>
                {sync.isSyncing ? 'Подключение...' : sync.isConnected ? 'Подключено' : 'Отключено'}
              </Text>
            </View>
            {sync.lastSyncAt && (
              <Text style={styles.syncMeta}>
                {new Date(sync.lastSyncAt).toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}
              </Text>
            )}
          </View>
        </CardRow>
      </Card>

      <Pressable
        style={[styles.saveBtn, sync.isConnected && styles.disconnectBtn]}
        onPress={() => {
          if (sync.isConnected) {
            sync.disconnect();
          } else {
            if (!syncUrl || !syncKey) {
              Alert.alert('Ошибка', 'Укажите сервер и API key');
              return;
            }
            sync.configureServer(syncUrl, syncKey);
            sync.connect();
          }
        }}
      >
        <Text style={styles.saveBtnText}>
          {sync.isConnected ? 'Отключиться' : 'Подключиться'}
        </Text>
      </Pressable>

      <Card noPadding style={styles.sectionCard}>
        <CardRow first>
          <Text style={styles.aboutText}>Elysium — трекер питания.{'\n'}Часть экосистемы SelfSuite.</Text>
        </CardRow>
        <CardRow>
          <Text style={styles.rowLabel}>Версия</Text>
          <Text style={styles.versionText}>1.0.0</Text>
        </CardRow>
      </Card>

      <View style={{ height: 40 }} />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: { flex: 1, backgroundColor: colors.bg.primary },
  content: {},
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: spacing.lg,
    paddingHorizontal: PAD,
  },
  backBtn: {
    width: 36,
    height: 36,
    borderRadius: 12,
    backgroundColor: colors.bg.card,
    justifyContent: 'center',
    alignItems: 'center',
  },
  headerTitle: {
    fontFamily: fonts.bold,
    fontSize: fontSize.lg,
    color: colors.text.primary,
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
  saveBtn: {
    backgroundColor: colors.accent,
    borderRadius: cardRadius,
    paddingVertical: spacing.md,
    alignItems: 'center',
    marginVertical: spacing.sm,
  },
  saveBtnText: { fontSize: fontSize.md, fontFamily: fonts.bold, color: colors.text.primary },
  sectionLabel: {
    fontSize: fontSize.xs,
    fontFamily: fonts.bold,
    color: colors.text.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    marginBottom: spacing.sm,
    marginTop: spacing.md,
    paddingHorizontal: PAD,
  },
  sourceRow: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  sourceInfo: { flex: 1 },
  sectionCard: { marginBottom: spacing.sm },
  sourceDesc: { fontSize: fontSize.xs, fontFamily: fonts.regular, color: colors.text.muted, marginTop: 2 },
  aboutText: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.secondary, lineHeight: 22 },
  computed: { fontSize: fontSize.md, fontFamily: fonts.monoBold, color: colors.text.muted },
  versionText: { fontSize: fontSize.md, fontFamily: fonts.mono, color: colors.text.muted },
  syncField: { flex: 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' },
  syncInput: { width: 160, textAlign: 'right' },
  syncStatusRow: { flex: 1, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' },
  syncStatusLeft: { flexDirection: 'row', alignItems: 'center', gap: 8 },
  dot: { width: 8, height: 8, borderRadius: 4 },
  dotOn: { backgroundColor: colors.accent },
  dotSyncing: { backgroundColor: '#FACC15' },
  dotOff: { backgroundColor: colors.text.muted },
  syncMeta: { fontSize: fontSize.xs, fontFamily: fonts.mono, color: colors.text.muted },
  disconnectBtn: { backgroundColor: colors.over },
});
