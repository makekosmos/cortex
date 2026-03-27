import React, { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { View, StyleSheet, Text, TextInput, FlatList, Pressable, KeyboardAvoidingView, Platform, ActivityIndicator } from 'react-native';
import { useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, radius, fonts } from '@/theme';
import { useFoodStore } from '@/stores/food-store';
import { useSettingsStore } from '@/stores/settings-store';
import { searchOpenFoodFacts } from '@/api/open-food-facts';
import { searchFatSecret } from '@/api/fatsecret';
import type { FoodItem } from '@/types/nutrition';
import { formatCalories, formatMacro } from '@/utils/macros';

interface FoodSearchProps { onSelect: (food: FoodItem) => void; onCreateNew: () => void; mealType?: string; }
type ListItem = { type: 'header'; title: string; key: string } | { type: 'food'; food: FoodItem; key: string };

export function FoodSearch({ onSelect, onCreateNew, mealType }: FoodSearchProps) {
  const router = useRouter();
  const [query, setQuery] = useState('');
  const [offResults, setOffResults] = useState<FoodItem[]>([]);
  const [offLoading, setOffLoading] = useState(false);
  const foodSource = useSettingsStore((s) => s.foodSource);
  const searchFoods = useFoodStore((s) => s.searchFoods);
  const getRecent = useFoodStore((s) => s.getRecent);
  const localResults = useMemo(() => searchFoods(query), [query, searchFoods]);
  const recentFoods = useMemo(() => getRecent(), [getRecent]);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    const q = query.trim();
    if (q.length < 3) { setOffResults([]); setOffLoading(false); return; }
    setOffLoading(true);
    debounceRef.current = setTimeout(async () => {
      const searchFn = foodSource === 'fatsecret' ? searchFatSecret : searchOpenFoodFacts;
      const r = await searchFn(q);
      setOffResults(r);
      setOffLoading(false);
    }, 500);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [query, foodSource]);

  const isSearching = query.trim().length > 0;

  const listItems = useMemo<ListItem[]>(() => {
    if (isSearching) {
      const seen = new Set<string>();
      const items: ListItem[] = [];
      for (const f of localResults) { const k = f.name.toLowerCase(); if (!seen.has(k)) { seen.add(k); items.push({ type: 'food', food: f, key: `l-${f.id}` }); } }
      for (const f of offResults) { const k = f.name.toLowerCase(); if (!seen.has(k)) { seen.add(k); items.push({ type: 'food', food: f, key: `o-${f.id}` }); } }
      return items;
    }
    const items: ListItem[] = [];
    if (recentFoods.length > 0) {
      items.push({ type: 'header', title: 'Недавние', key: 'h-recent' });
      for (const f of recentFoods) items.push({ type: 'food', food: f, key: `r-${f.id}-${f.name}` });
    }
    return items;
  }, [isSearching, localResults, offResults, recentFoods]);

  const renderItem = useCallback(({ item }: { item: ListItem }) => {
    if (item.type === 'header') return (
      <View style={styles.sectionHeader}>
        <Ionicons name={item.title === 'Недавние' ? 'time-outline' : 'list-outline'} size={14} color={colors.text.muted} />
        <Text style={styles.sectionTitle}>{item.title}</Text>
      </View>
    );
    const food = item.food;
    return (
      <Pressable style={styles.foodItem} onPress={() => onSelect(food)}>
        <View style={styles.foodInfo}>
          <Text style={styles.foodName}>{food.name}</Text>
          {food.brand && <Text style={styles.foodBrand}>{food.brand}</Text>}
          <Text style={styles.foodServing}>{food.servingSize} {food.servingUnit}</Text>
        </View>
        <View style={styles.foodMacros}>
          <Text style={styles.foodCal}>{formatCalories(food.macros.calories)}</Text>
          <Text style={styles.foodMacroLine}>Б {formatMacro(food.macros.protein)} · Ж {formatMacro(food.macros.fat)} · У {formatMacro(food.macros.carbs)}</Text>
        </View>
      </Pressable>
    );
  }, [onSelect]);

  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : undefined} style={styles.container}>
      <View style={styles.topRow}>
        <View style={styles.searchBar}>
          <Ionicons name="search" size={18} color={colors.text.muted} />
          <TextInput style={styles.input} placeholder="Найти продукт..." placeholderTextColor={colors.text.muted} value={query} onChangeText={setQuery} autoFocus returnKeyType="search" />
          {query.length > 0 && <Pressable onPress={() => setQuery('')} hitSlop={8}><Ionicons name="close-circle" size={18} color={colors.text.muted} /></Pressable>}
        </View>
        <Pressable style={styles.scanBtn} onPress={() => router.push({ pathname: '/scanner', params: { mealType: mealType ?? 'snack' } })} hitSlop={8}>
          <Ionicons name="barcode-outline" size={24} color={colors.accent} />
        </Pressable>
      </View>
      {offLoading && <View style={styles.loadingRow}><ActivityIndicator size="small" color={colors.accent} /><Text style={styles.loadingText}>Поиск в {foodSource === 'fatsecret' ? 'FatSecret' : 'Open Food Facts'}...</Text></View>}
      <FlatList
        data={listItems} keyExtractor={(i) => i.key} renderItem={renderItem}
        ListEmptyComponent={!offLoading ? (
          <View style={styles.empty}>
            <Ionicons name="nutrition-outline" size={48} color={colors.text.muted} style={{ marginBottom: spacing.md }} />
            <Text style={styles.emptyText}>{isSearching ? 'Ничего не найдено' : 'Нет недавних продуктов'}</Text>
            <Text style={styles.emptyHint}>Найдите продукт в поиске или отсканируйте штрих-код</Text>
            <View style={styles.emptyActions}>
              <Pressable style={styles.actionBtn} onPress={() => router.push({ pathname: '/scanner', params: { mealType: mealType ?? 'snack' } })}><Ionicons name="barcode-outline" size={20} color={colors.accent} /><Text style={styles.actionText}>Сканировать штрих-код</Text></Pressable>
              <Pressable style={styles.actionBtn} onPress={onCreateNew}><Ionicons name="add" size={20} color={colors.accent} /><Text style={styles.actionText}>Создать свой продукт</Text></Pressable>
            </View>
          </View>
        ) : null}
        ListFooterComponent={listItems.length > 0 ? (
          <View style={styles.footer}>
            <Pressable style={styles.actionBtn} onPress={() => router.push({ pathname: '/scanner', params: { mealType: mealType ?? 'snack' } })}><Ionicons name="barcode-outline" size={20} color={colors.accent} /><Text style={styles.actionText}>Сканировать штрих-код</Text></Pressable>
            <Pressable style={styles.actionBtn} onPress={onCreateNew}><Ionicons name="add" size={20} color={colors.accent} /><Text style={styles.actionText}>Создать свой продукт</Text></Pressable>
          </View>
        ) : null}
        contentContainerStyle={[styles.list, listItems.length === 0 && styles.listEmpty]}
      />
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  topRow: { flexDirection: 'row', alignItems: 'center', marginHorizontal: spacing.lg, marginBottom: spacing.md, gap: spacing.sm },
  searchBar: { flex: 1, flexDirection: 'row', alignItems: 'center', backgroundColor: colors.bg.input, borderRadius: radius.md, paddingHorizontal: spacing.md, height: 44, gap: spacing.sm },
  input: { flex: 1, fontSize: fontSize.md, fontFamily: fonts.regular, color: colors.text.primary },
  scanBtn: { width: 44, height: 44, backgroundColor: colors.bg.input, borderRadius: radius.md, justifyContent: 'center', alignItems: 'center' },
  loadingRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: spacing.sm, paddingBottom: spacing.sm },
  loadingText: { fontSize: fontSize.xs, color: colors.text.muted },
  list: { paddingHorizontal: spacing.lg },
  sectionHeader: { flexDirection: 'row', alignItems: 'center', gap: 6, paddingTop: spacing.lg, paddingBottom: spacing.sm },
  sectionTitle: { fontSize: fontSize.xs, fontFamily: fonts.bold, color: colors.text.muted, textTransform: 'uppercase', letterSpacing: 0.5 },
  foodItem: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', paddingVertical: spacing.md, borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: colors.separator },
  foodInfo: { flex: 1, marginRight: spacing.md },
  foodName: { fontSize: fontSize.md, fontFamily: fonts.regular, color: colors.text.primary },
  foodBrand: { fontSize: fontSize.xs, color: colors.text.muted, marginTop: 2 },
  foodServing: { fontSize: fontSize.xs, color: colors.text.secondary, marginTop: 2 },
  foodMacros: { alignItems: 'flex-end' },
  foodCal: { fontSize: fontSize.md, fontFamily: fonts.monoBold, color: colors.text.primary },
  foodMacroLine: { fontSize: fontSize.xs, fontFamily: fonts.mono, color: colors.text.muted, marginTop: 2 },
  listEmpty: { flexGrow: 1 },
  empty: { flex: 1, justifyContent: 'center', alignItems: 'center', paddingVertical: spacing.xxl },
  emptyText: { fontSize: fontSize.md, fontFamily: fonts.semiBold, color: colors.text.secondary, textAlign: 'center' },
  emptyHint: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.muted, textAlign: 'center', marginTop: spacing.xs },
  emptyActions: { width: '100%', paddingHorizontal: spacing.lg, marginTop: spacing.xl, gap: spacing.sm },
  footer: { paddingVertical: spacing.md, gap: spacing.sm },
  actionBtn: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: spacing.sm, paddingVertical: spacing.md, backgroundColor: colors.bg.card, borderRadius: radius.md },
  actionText: { fontSize: fontSize.md, fontFamily: fonts.regular, color: colors.accent },
});
