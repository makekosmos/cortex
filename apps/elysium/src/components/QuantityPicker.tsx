import React, { useState, useEffect } from 'react';
import { View, ScrollView, StyleSheet, Text, Pressable, TextInput } from 'react-native';
import { LinearGradient } from 'expo-linear-gradient';
import { Ionicons } from '@expo/vector-icons';
import { fetchFatSecretDetails } from '@/api/fatsecret';
import { colors, fontSize, spacing, fonts, cardRadius } from '@/theme';
import type { FoodItem, MealType } from '@/types/nutrition';
import { MEAL_TYPE_LABELS } from '@/types/nutrition';
import { formatCalories, formatMacro } from '@/utils/macros';
import { SegmentedControl } from './SegmentedControl';
import { MacroCard } from './MacroCard';

type Mode = 'servings' | 'grams';
const MODES: Mode[] = ['servings', 'grams'];
const MODE_LABELS = ['Порции', 'Граммы'];

interface Props {
  food: FoodItem;
  mealType?: MealType;
  initialQuantity?: number;
  confirmLabel?: string;
  onConfirm: (q: number, meal: MealType) => void;
  onBack: () => void;
}

function isWeightUnit(unit: string): boolean {
  const u = unit.toLowerCase().trim();
  return u === 'г' || u === 'мл' || u === 'g' || u === 'ml';
}

function gramsPerServing(food: FoodItem): number {
  if (isWeightUnit(food.servingUnit)) return food.servingSize;
  return 100;
}

// ── Food rating ──

interface Rating {
  score: 'good' | 'ok' | 'bad';
  label: string;
  color: string;
  reasons: string[];
}

function rateFood(food: FoodItem): Rating {
  const m = food.macros;
  const reasons: string[] = [];
  let badCount = 0;
  const w = isWeightUnit(food.servingUnit) && food.servingSize > 0;

  const calPer100 = w ? (m.calories / food.servingSize) * 100 : m.calories;
  if (calPer100 > 400) { reasons.push(`Высокая калорийность: ${Math.round(calPer100)} ккал${w ? '/100г' : ''}`); badCount++; }
  else if (calPer100 > 250) { reasons.push(`Средняя калорийность: ${Math.round(calPer100)} ккал${w ? '/100г' : ''}`); }

  const fatPer100 = w ? (m.fat / food.servingSize) * 100 : m.fat;
  const carbPer100 = w ? (m.carbs / food.servingSize) * 100 : m.carbs;
  if (fatPer100 > 15 && carbPer100 > 30) { reasons.push('Много жиров и углеводов одновременно'); badCount++; }

  const protPer100 = w ? (m.protein / food.servingSize) * 100 : m.protein;
  if (protPer100 > 20) reasons.push(`Высокое содержание белка: ${Math.round(protPer100)}г${w ? '/100г' : ''}`);
  if (calPer100 > 0 && calPer100 < 100) reasons.push('Низкокалорийный продукт');

  if (badCount >= 2) return { score: 'bad', label: 'Не лучший выбор', color: colors.over, reasons };
  if (badCount === 1) return { score: 'ok', label: 'Умеренно', color: '#FACC15', reasons };
  return { score: 'good', label: 'Хороший выбор', color: '#4ADE80', reasons };
}

// ── Component ──

export function QuantityPicker({ food: initialFood, mealType, initialQuantity = 1, confirmLabel = 'Добавить', onConfirm, onBack }: Props) {
  const [food, setFood] = useState(initialFood);
  const gps = gramsPerServing(food);
  const defaultMode: Mode = isWeightUnit(food.servingUnit) ? 'grams' : 'servings';

  const [mode, setMode] = useState<Mode>(defaultMode);
  const [selectedMeal, setSelectedMeal] = useState<MealType>(mealType ?? 'snack');
  const [servings, setServings] = useState(initialQuantity);
  const [servingsText, setServingsText] = useState(String(initialQuantity));
  const [grams, setGrams] = useState(Math.round(initialQuantity * gps));
  const [gramsText, setGramsText] = useState(String(Math.round(initialQuantity * gps)));

  useEffect(() => {
    if (initialFood.id.startsWith('fs-') && !initialFood.nutrients) {
      fetchFatSecretDetails(initialFood).then(setFood);
    }
  }, [initialFood]);

  const multiplier = mode === 'servings' ? servings : gps > 0 ? grams / gps : 0;
  const cal = food.macros.calories * multiplier;
  const p = food.macros.protein * multiplier;
  const f = food.macros.fat * multiplier;
  const c = food.macros.carbs * multiplier;
  const rating = rateFood(food);

  const switchMode = (idx: number) => {
    const m = MODES[idx];
    if (m === mode) return;
    if (m === 'grams') {
      const g = Math.round(servings * gps);
      setGrams(g); setGramsText(String(g));
    } else {
      const s = gps > 0 ? Math.round((grams / gps) * 10) / 10 : 1;
      setServings(s); setServingsText(String(s));
    }
    setMode(m);
  };

  const step = mode === 'servings' ? 0.5 : 10;
  const increment = () => {
    if (mode === 'servings') { const n = servings + step; setServings(n); setServingsText(n.toString()); }
    else { const n = grams + step; setGrams(n); setGramsText(Math.round(n).toString()); }
  };
  const decrement = () => {
    if (mode === 'servings') { const n = Math.max(0.5, servings - step); setServings(n); setServingsText(n.toString()); }
    else { const n = Math.max(1, grams - step); setGrams(n); setGramsText(Math.round(n).toString()); }
  };

  const unitLabel = mode === 'servings'
    ? (isWeightUnit(food.servingUnit) ? 'порций' : food.servingUnit)
    : food.servingUnit || 'г';

  // Macros for MacroCard (same as main screen)
  const macros = [
    { label: 'Углеводы', current: c, goal: 0, color: colors.macro.carbs },
    { label: 'Белки', current: p, goal: 0, color: colors.macro.protein },
    { label: 'Жиры', current: f, goal: 0, color: colors.macro.fat },
  ];

  return (
    <View style={styles.root}>
      <ScrollView style={styles.scroll} contentContainerStyle={styles.container} showsVerticalScrollIndicator={false}>
        <Pressable onPress={onBack} style={styles.backBtn} hitSlop={12}>
          <Ionicons name="arrow-back" size={22} color={colors.text.primary} />
        </Pressable>

        <Text style={styles.foodName}>{food.name}</Text>
        {food.brand && <Text style={styles.brand}>{food.brand}</Text>}
        <Text style={styles.servingInfo}>{food.servingSize} {food.servingUnit} · {formatCalories(food.macros.calories)} ккал на порцию</Text>

        {/* Quantity input */}
        <View style={styles.inputRow}>
          <Pressable onPress={decrement} style={styles.qBtn}>
            <Ionicons name="remove" size={20} color={colors.text.primary} />
          </Pressable>
          <TextInput
            style={styles.qInput}
            value={mode === 'servings' ? servingsText : gramsText}
            onChangeText={(t) => {
              if (mode === 'servings') { setServingsText(t); const v = parseFloat(t); if (!isNaN(v) && v > 0) setServings(v); }
              else { setGramsText(t); const v = parseFloat(t); if (!isNaN(v) && v > 0) setGrams(v); }
            }}
            keyboardType="decimal-pad"
            textAlign="center"
          />
          <Pressable onPress={increment} style={styles.qBtn}>
            <Ionicons name="add" size={20} color={colors.text.primary} />
          </Pressable>
        </View>
        <Text style={styles.unitLabel}>{unitLabel}</Text>

        {/* Mode toggle — same animated style as WeekChart tabs */}
        <SegmentedControl
          segments={MODE_LABELS}
          activeIndex={MODES.indexOf(mode)}
          onChange={switchMode}
        />

        {/* Meal type */}
        <View style={styles.mealSection}>
          {(['breakfast', 'lunch', 'dinner', 'snack'] as MealType[]).map((mt) => (
            <Pressable key={mt} style={[styles.mealBtn, selectedMeal === mt && styles.mealBtnActive]} onPress={() => setSelectedMeal(mt)}>
              <Text style={[styles.mealText, selectedMeal === mt && styles.mealTextActive]}>{MEAL_TYPE_LABELS[mt]}</Text>
            </Pressable>
          ))}
        </View>

        {/* Calories */}
        <View style={styles.calCard}>
          <Text style={styles.calValue}>{formatCalories(cal)}</Text>
          <Text style={styles.calLabel}>ккал</Text>
        </View>

        {/* Macros — same grid with squares as main screen */}
        <MacroCard macros={macros} />

        {/* Rating */}
        <View style={styles.ratingCard}>
          <View style={styles.ratingHeader}>
            <View style={[styles.ratingDot, { backgroundColor: rating.color }]} />
            <Text style={[styles.ratingLabel, { color: rating.color }]}>{rating.label}</Text>
          </View>
          {rating.reasons.map((r, i) => (
            <Text key={i} style={styles.ratingReason}>• {r}</Text>
          ))}
        </View>

        {/* Extended nutrients */}
        {food.nutrients && Object.values(food.nutrients).some((v) => v !== undefined && v > 0) && (
          <View style={styles.nutrientsCard}>
            <Text style={styles.sectionTitle}>Дополнительно</Text>
            {food.nutrients.saturatedFat != null && food.nutrients.saturatedFat > 0 && <NutrientRow label="Насыщ. жиры" value={`${Math.round(food.nutrients.saturatedFat * multiplier * 10) / 10}г`} />}
            {food.nutrients.transFat != null && food.nutrients.transFat > 0 && <NutrientRow label="Транс-жиры" value={`${Math.round(food.nutrients.transFat * multiplier * 10) / 10}г`} />}
            {food.nutrients.sugar != null && food.nutrients.sugar > 0 && <NutrientRow label="Сахар" value={`${Math.round(food.nutrients.sugar * multiplier * 10) / 10}г`} />}
            {food.nutrients.fiber != null && food.nutrients.fiber > 0 && <NutrientRow label="Клетчатка" value={`${Math.round(food.nutrients.fiber * multiplier * 10) / 10}г`} />}
            {food.nutrients.cholesterol != null && food.nutrients.cholesterol > 0 && <NutrientRow label="Холестерин" value={`${Math.round(food.nutrients.cholesterol * multiplier)}мг`} />}
            {food.nutrients.sodium != null && food.nutrients.sodium > 0 && <NutrientRow label="Натрий" value={`${Math.round(food.nutrients.sodium * multiplier)}мг`} />}
            {food.nutrients.potassium != null && food.nutrients.potassium > 0 && <NutrientRow label="Калий" value={`${Math.round(food.nutrients.potassium * multiplier)}мг`} />}
          </View>
        )}

        <View style={{ height: 100 }} />
      </ScrollView>

      {/* Sticky button with gradient fade like navbar */}
      <View style={styles.stickyWrap} pointerEvents="box-none">
        <LinearGradient
          colors={['transparent', 'rgba(10,10,15,0.85)', 'rgba(10,10,15,1)']}
          style={styles.stickyGradient}
          pointerEvents="none"
        />
        <View style={styles.stickyInner}>
          <Pressable style={styles.confirmBtn} onPress={() => onConfirm(multiplier, selectedMeal)}>
            <Text style={styles.confirmText}>{confirmLabel}</Text>
          </Pressable>
        </View>
      </View>
    </View>
  );
}

function NutrientRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.nutrientRow}>
      <Text style={styles.nutrientLabel}>{label}</Text>
      <Text style={styles.nutrientValue}>{value}</Text>
    </View>
  );
}

const PAD = spacing.lg;

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: colors.bg.primary },
  scroll: { flex: 1 },
  container: { paddingTop: spacing.lg },

  backBtn: { marginBottom: spacing.md, alignSelf: 'flex-start', paddingHorizontal: PAD },
  foodName: { fontSize: fontSize.xl, fontFamily: fonts.bold, color: colors.text.primary, paddingHorizontal: PAD },
  brand: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.muted, marginTop: 2, paddingHorizontal: PAD },
  servingInfo: { fontSize: fontSize.xs, fontFamily: fonts.mono, color: colors.text.muted, marginTop: 4, paddingHorizontal: PAD, marginBottom: spacing.lg },

  inputRow: { flexDirection: 'row', alignItems: 'center', gap: spacing.md, paddingHorizontal: PAD },
  qBtn: { width: 44, height: 44, borderRadius: 22, backgroundColor: colors.bg.card, justifyContent: 'center', alignItems: 'center' },
  qInput: { flex: 1, height: 56, backgroundColor: colors.bg.card, borderRadius: cardRadius, fontSize: fontSize.xxl, fontFamily: fonts.monoBold, color: colors.text.primary, paddingHorizontal: spacing.md },
  unitLabel: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.muted, textAlign: 'center', marginTop: spacing.xs, marginBottom: spacing.md },

  mealSection: { flexDirection: 'row', gap: spacing.xs, marginTop: spacing.xs },
  mealBtn: { flex: 1, paddingVertical: spacing.md, backgroundColor: colors.bg.card, borderRadius: cardRadius, alignItems: 'center' },
  mealBtnActive: { backgroundColor: colors.accent },
  mealText: { fontSize: fontSize.xs, fontFamily: fonts.semiBold, color: colors.text.muted },
  mealTextActive: { color: colors.text.primary },

  calCard: { backgroundColor: colors.bg.card, borderRadius: cardRadius, padding: PAD, marginTop: spacing.xs, alignItems: 'center' },
  calValue: { fontSize: fontSize.hero, fontFamily: fonts.monoBold, color: colors.text.primary },
  calLabel: { fontSize: fontSize.xs, fontFamily: fonts.regular, color: colors.text.muted, marginTop: 2 },

  ratingCard: { backgroundColor: colors.bg.card, borderRadius: cardRadius, padding: PAD, marginTop: spacing.xs },
  ratingHeader: { flexDirection: 'row', alignItems: 'center', gap: spacing.sm, marginBottom: spacing.sm },
  ratingDot: { width: 10, height: 10, borderRadius: 5 },
  ratingLabel: { fontSize: fontSize.md, fontFamily: fonts.semiBold },
  ratingReason: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.secondary, marginTop: 4 },

  nutrientsCard: { backgroundColor: colors.bg.card, borderRadius: cardRadius, padding: PAD, marginTop: spacing.xs },
  sectionTitle: { fontSize: fontSize.xs, fontFamily: fonts.semiBold, color: colors.text.muted, textTransform: 'uppercase', letterSpacing: 0.5, marginBottom: spacing.sm },
  nutrientRow: { flexDirection: 'row', justifyContent: 'space-between', paddingVertical: 6 },
  nutrientLabel: { fontSize: fontSize.sm, fontFamily: fonts.regular, color: colors.text.secondary },
  nutrientValue: { fontSize: fontSize.sm, fontFamily: fonts.mono, color: colors.text.primary },

  stickyWrap: { position: 'absolute', bottom: 0, left: 0, right: 0 },
  stickyGradient: { height: 100, position: 'absolute', bottom: 0, left: 0, right: 0 },
  stickyInner: { paddingHorizontal: PAD, paddingBottom: PAD },
  confirmBtn: { backgroundColor: colors.accent, borderRadius: cardRadius, paddingVertical: spacing.md, alignItems: 'center' },
  confirmText: { fontSize: fontSize.lg, fontFamily: fonts.bold, color: colors.text.primary },
});
