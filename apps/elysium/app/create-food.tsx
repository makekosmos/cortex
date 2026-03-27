import React, { useState } from 'react';
import { View, ScrollView, StyleSheet, Text, TextInput, Pressable, Alert } from 'react-native';
import { useRouter } from 'expo-router';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, radius, fonts } from '@/theme';
import { useFoodStore } from '@/stores/food-store';

export default function CreateFoodScreen() {
  const router = useRouter();
  const addCustomFood = useFoodStore((s) => s.addCustomFood);
  const [name, setName] = useState('');
  const [brand, setBrand] = useState('');
  const [servingSize, setServingSize] = useState('100');
  const [servingUnit, setServingUnit] = useState('г');
  const [calories, setCalories] = useState('');
  const [protein, setProtein] = useState('');
  const [fat, setFat] = useState('');
  const [carbs, setCarbs] = useState('');

  const handleSave = () => {
    if (!name.trim()) { Alert.alert('Ошибка', 'Введите название'); return; }
    addCustomFood({ name: name.trim(), brand: brand.trim() || undefined, servingSize: parseFloat(servingSize) || 100, servingUnit, macros: { calories: parseFloat(calories) || 0, protein: parseFloat(protein) || 0, fat: parseFloat(fat) || 0, carbs: parseFloat(carbs) || 0 } });
    router.back();
  };

  return (
    <SafeAreaView style={styles.safe} edges={['top']}>
      <ScrollView contentContainerStyle={styles.content}>
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={12}><Ionicons name="close" size={24} color={colors.text.primary} /></Pressable>
          <Text style={styles.title}>Новый продукт</Text>
          <View style={{ width: 24 }} />
        </View>
        <Field label="Название" value={name} onChangeText={setName} placeholder="Курица гриль" />
        <Field label="Бренд" value={brand} onChangeText={setBrand} placeholder="Опционально" />
        <View style={{ flexDirection: 'row', gap: spacing.sm }}>
          <View style={{ flex: 2 }}><Field label="Порция" value={servingSize} onChangeText={setServingSize} keyboardType="decimal-pad" /></View>
          <View style={{ flex: 1 }}><Field label="Ед." value={servingUnit} onChangeText={setServingUnit} /></View>
        </View>
        <Text style={styles.sectionLabel}>Нутриенты (на порцию)</Text>
        <Field label="Калории" value={calories} onChangeText={setCalories} keyboardType="decimal-pad" placeholder="0" />
        <Field label="Белки (г)" value={protein} onChangeText={setProtein} keyboardType="decimal-pad" placeholder="0" />
        <Field label="Жиры (г)" value={fat} onChangeText={setFat} keyboardType="decimal-pad" placeholder="0" />
        <Field label="Углеводы (г)" value={carbs} onChangeText={setCarbs} keyboardType="decimal-pad" placeholder="0" />
        <Pressable style={styles.saveBtn} onPress={handleSave}><Text style={styles.saveBtnText}>Сохранить</Text></Pressable>
      </ScrollView>
    </SafeAreaView>
  );
}

function Field({ label, value, onChangeText, placeholder, keyboardType }: { label: string; value: string; onChangeText: (t: string) => void; placeholder?: string; keyboardType?: 'default' | 'decimal-pad' }) {
  return (
    <View style={fStyles.wrap}>
      <Text style={fStyles.label}>{label}</Text>
      <TextInput style={fStyles.input} value={value} onChangeText={onChangeText} placeholder={placeholder} placeholderTextColor={colors.text.muted} keyboardType={keyboardType ?? 'default'} />
    </View>
  );
}

const fStyles = StyleSheet.create({
  wrap: { marginBottom: spacing.md },
  label: { fontSize: fontSize.xs, color: colors.text.secondary, marginBottom: 4 },
  input: { backgroundColor: colors.bg.input, borderRadius: radius.sm, paddingHorizontal: spacing.md, paddingVertical: spacing.sm, fontSize: fontSize.md, color: colors.text.primary, height: 44 },
});

const styles = StyleSheet.create({
  safe: { flex: 1, backgroundColor: colors.bg.primary },
  content: { paddingHorizontal: spacing.lg, paddingBottom: 40 },
  header: { flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', paddingVertical: spacing.md },
  title: { fontSize: fontSize.lg, fontFamily: fonts.bold, color: colors.text.primary },
  sectionLabel: { fontSize: fontSize.sm, fontFamily: fonts.semiBold, color: colors.text.secondary, marginBottom: spacing.sm, marginTop: spacing.sm },
  saveBtn: { backgroundColor: colors.accent, borderRadius: radius.md, paddingVertical: spacing.md, alignItems: 'center', marginTop: spacing.lg },
  saveBtnText: { fontSize: fontSize.lg, fontFamily: fonts.bold, color: colors.text.primary },
});
