import { useState } from 'react';
import {
  View, Text, StyleSheet, TextInput, TouchableOpacity, ScrollView, Alert,
} from 'react-native';
import { router } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { useThemeColor } from '@/lib/useThemeColor';
import { createCustomExercise } from '@/lib/database';
import { useExercisesStore } from '@/lib/stores/exercises-store';
import { generateId } from '@/lib/id';
import { MUSCLE_GROUPS, EQUIPMENT_LIST, muscleRu } from '@/lib/types';

const EQUIPMENT_RU: Record<string, string> = {
  'barbell': 'Штанга',
  'dumbbell': 'Гантели',
  'kettlebells': 'Гири',
  'machine': 'Тренажёр',
  'cable': 'Блок',
  'body only': 'Своё тело',
  'bands': 'Резинки',
  'foam roll': 'Ролик',
  'e-z curl bar': 'EZ-гриф',
  'medicine ball': 'Медбол',
  'exercise ball': 'Фитбол',
  'other': 'Другое',
};

export default function CreateExerciseScreen() {
  const colors = useThemeColor();
  const reload = useExercisesStore((s) => s.reload);
  const [name, setName] = useState('');
  const [selectedMuscle, setSelectedMuscle] = useState<string | null>(null);
  const [selectedEquipment, setSelectedEquipment] = useState<string | null>(null);
  const [instructions, setInstructions] = useState('');
  const [saving, setSaving] = useState(false);

  async function handleSave() {
    const trimmedName = name.trim();
    if (!trimmedName) {
      Alert.alert('Ошибка', 'Введите название упражнения');
      return;
    }
    if (!selectedMuscle) {
      Alert.alert('Ошибка', 'Выберите группу мышц');
      return;
    }

    setSaving(true);
    try {
      await createCustomExercise({
        id: generateId(),
        name: trimmedName,
        primary_muscles: [selectedMuscle],
        equipment: selectedEquipment,
        instructions: instructions.trim() ? [instructions.trim()] : [],
      });
      await reload();
      router.back();
    } catch (e) {
      Alert.alert('Ошибка', 'Не удалось сохранить упражнение');
    } finally {
      setSaving(false);
    }
  }

  return (
    <View style={[styles.container, { backgroundColor: colors.background }]}>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        {/* Name */}
        <Text style={[styles.label, { color: colors.textSecondary }]}>Название</Text>
        <TextInput
          style={[styles.input, { backgroundColor: colors.surface, color: colors.text, borderColor: colors.border }]}
          placeholder="Например: Жим сидя в Смите"
          placeholderTextColor={colors.textTertiary}
          value={name}
          onChangeText={setName}
          autoFocus
          autoCorrect={false}
        />

        {/* Muscle group */}
        <Text style={[styles.label, { color: colors.textSecondary }]}>Группа мышц</Text>
        <View style={styles.chipContainer}>
          {MUSCLE_GROUPS.map((m) => (
            <TouchableOpacity
              key={m}
              style={[styles.chip, { backgroundColor: selectedMuscle === m ? colors.accent : colors.surface }]}
              onPress={() => setSelectedMuscle(selectedMuscle === m ? null : m)}
            >
              <Text style={[styles.chipText, { color: selectedMuscle === m ? '#fff' : colors.textSecondary }]}>
                {muscleRu(m)}
              </Text>
            </TouchableOpacity>
          ))}
        </View>

        {/* Equipment */}
        <Text style={[styles.label, { color: colors.textSecondary }]}>Оборудование</Text>
        <View style={styles.chipContainer}>
          {EQUIPMENT_LIST.map((eq) => (
            <TouchableOpacity
              key={eq}
              style={[styles.chip, { backgroundColor: selectedEquipment === eq ? colors.accent : colors.surface }]}
              onPress={() => setSelectedEquipment(selectedEquipment === eq ? null : eq)}
            >
              <Text style={[styles.chipText, { color: selectedEquipment === eq ? '#fff' : colors.textSecondary }]}>
                {EQUIPMENT_RU[eq] || eq}
              </Text>
            </TouchableOpacity>
          ))}
        </View>

        {/* Instructions */}
        <Text style={[styles.label, { color: colors.textSecondary }]}>Описание (необязательно)</Text>
        <TextInput
          style={[styles.textArea, { backgroundColor: colors.surface, color: colors.text, borderColor: colors.border }]}
          placeholder="Техника выполнения, заметки..."
          placeholderTextColor={colors.textTertiary}
          value={instructions}
          onChangeText={setInstructions}
          multiline
          numberOfLines={4}
          textAlignVertical="top"
        />

        <View style={{ height: 20 }} />
      </ScrollView>

      {/* Save button */}
      <View style={[styles.bottomBar, { backgroundColor: colors.surface, borderTopColor: colors.border }]}>
        <TouchableOpacity
          style={[styles.saveBtn, { backgroundColor: colors.accent, opacity: saving ? 0.6 : 1 }]}
          onPress={handleSave}
          disabled={saving}
          activeOpacity={0.7}
        >
          <Ionicons name="checkmark" size={20} color="#fff" />
          <Text style={styles.saveBtnText}>Сохранить</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },
  content: { padding: 16 },
  label: { fontSize: 14, fontWeight: '600', marginBottom: 8, marginTop: 16 },
  input: {
    fontSize: 16, paddingHorizontal: 14, paddingVertical: 12,
    borderRadius: 10, borderWidth: StyleSheet.hairlineWidth,
  },
  textArea: {
    fontSize: 15, paddingHorizontal: 14, paddingVertical: 12,
    borderRadius: 10, borderWidth: StyleSheet.hairlineWidth,
    minHeight: 100,
  },
  chipContainer: { flexDirection: 'row', flexWrap: 'wrap', gap: 8 },
  chip: { paddingHorizontal: 14, paddingVertical: 8, borderRadius: 16 },
  chipText: { fontSize: 13, fontWeight: '500' },
  bottomBar: {
    padding: 16, paddingBottom: 32, borderTopWidth: StyleSheet.hairlineWidth,
  },
  saveBtn: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center',
    paddingVertical: 14, borderRadius: 10, gap: 8,
  },
  saveBtnText: { color: '#fff', fontSize: 16, fontWeight: '700' },
});
