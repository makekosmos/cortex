import { useState, useMemo, useCallback, useEffect } from "react";

import {
  View,
  Text,
  StyleSheet,
  TextInput,
  TouchableOpacity,
  FlatList,
  ScrollView,
  Alert,
  ActivityIndicator,
} from "react-native";

import { router } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { generateId } from "@/lib/id";

import { useThemeColor } from "@/lib/useThemeColor";

import { useExercisesStore } from "@/lib/stores/exercises-store";

import { saveRoutine } from "@/lib/database";

import { MUSCLE_GROUPS, musclesRu, muscleRu } from "@/lib/types";

const PAGE_SIZE = 30;

export default function CreateRoutineScreen() {
  const colors = useThemeColor();

  const allExercises = useExercisesStore((s) => s.exercises);

  const [title, setTitle] = useState("");

  const [step, setStep] = useState<"name" | "exercises">("name");

  const [search, setSearch] = useState("");

  const [selectedMuscle, setSelectedMuscle] = useState<string | null>(null);

  const [selectedExercises, setSelectedExercises] = useState<any[]>([]);

  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  useEffect(() => {
    setVisibleCount(PAGE_SIZE);
  }, [search, selectedMuscle]);

  const filtered = useMemo(() => {
    let result = allExercises;

    if (search) {
      const q = search.toLowerCase();

      result = result.filter((e) => e.name.toLowerCase().includes(q));
    }

    if (selectedMuscle) {
      result = result.filter((e) => e.primary_muscles.includes(selectedMuscle));
    }

    return result;
  }, [allExercises, search, selectedMuscle]);

  const visible = useMemo(
    () => filtered.slice(0, visibleCount),
    [filtered, visibleCount],
  );

  const hasMore = visibleCount < filtered.length;

  const loadMore = useCallback(() => {
    if (hasMore) setVisibleCount((prev) => prev + PAGE_SIZE);
  }, [hasMore]);

  function toggleExercise(ex: any) {
    setSelectedExercises((prev) => {
      const exists = prev.find((e) => e.id === ex.id);

      if (exists) return prev.filter((e) => e.id !== ex.id);

      return [...prev, ex];
    });
  }

  async function handleSave() {
    if (!title.trim()) {
      Alert.alert("Введите название");
      return;
    }

    if (selectedExercises.length === 0) {
      Alert.alert("Добавьте хотя бы одно упражнение");
      return;
    }

    const routineExercises = selectedExercises.map((ex, i) => ({
      id: generateId(),
      exercise_id: ex.id,
      sort_order: i,

      target_sets: 3,
      target_reps: "8-12",
      target_weight_kg: null,
      notes: "",
    }));

    await saveRoutine(
      { id: generateId(), title: title.trim() },
      routineExercises,
    );

    router.back();
  }

  if (step === "name") {
    return (
      <View style={[styles.container, { backgroundColor: colors.background }]}>
        <Text style={[styles.label, { color: colors.text }]}>
          Название программы
        </Text>
        <TextInput
          style={[
            styles.titleInput,
            { color: colors.text, backgroundColor: colors.surface },
          ]}
          value={title}
          onChangeText={setTitle}
          placeholder="напр. День жима, Верх тела"
          placeholderTextColor={colors.textTertiary}
          autoFocus
        />
        <TouchableOpacity
          style={[
            styles.nextBtn,
            { backgroundColor: colors.accent, opacity: title.trim() ? 1 : 0.5 },
          ]}
          onPress={() => title.trim() && setStep("exercises")}
          disabled={!title.trim()}
        >
          <Text style={styles.nextBtnText}>Далее: выбрать упражнения</Text>
        </TouchableOpacity>
      </View>
    );
  }

  return (
    <View style={[styles.container, { backgroundColor: colors.background }]}>
      <View
        style={[styles.searchContainer, { backgroundColor: colors.surface }]}
      >
        <Ionicons name="search" size={18} color={colors.textTertiary} />
        <TextInput
          style={[styles.searchInput, { color: colors.text }]}
          placeholder="Поиск..."
          placeholderTextColor={colors.textTertiary}
          value={search}
          onChangeText={setSearch}
          autoCorrect={false}
        />
      </View>

      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        style={styles.filterRow}
      >
        {MUSCLE_GROUPS.map((m) => (
          <TouchableOpacity
            key={m}
            style={[
              styles.chip,
              {
                backgroundColor:
                  selectedMuscle === m ? colors.accent : colors.surface,
              },
            ]}
            onPress={() => setSelectedMuscle(selectedMuscle === m ? null : m)}
          >
            <Text
              style={[
                styles.chipText,
                { color: selectedMuscle === m ? "#fff" : colors.textSecondary },
              ]}
            >
              {muscleRu(m)}
            </Text>
          </TouchableOpacity>
        ))}
      </ScrollView>

      <FlatList
        data={visible}
        keyExtractor={(item) => item.id}
        contentContainerStyle={{ paddingHorizontal: 16, paddingBottom: 100 }}
        renderItem={({ item }) => {
          const isSelected = selectedExercises.some((e) => e.id === item.id);

          return (
            <TouchableOpacity
              style={[styles.exerciseRow, { borderBottomColor: colors.border }]}
              onPress={() => toggleExercise(item)}
              activeOpacity={0.6}
            >
              <View style={{ flex: 1 }}>
                <Text style={[styles.exerciseName, { color: colors.text }]}>
                  {item.name}
                </Text>
                <Text
                  style={[styles.exerciseMeta, { color: colors.textSecondary }]}
                >
                  {musclesRu(item.primary_muscles)}
                </Text>
              </View>
              <Ionicons
                name={isSelected ? "checkmark-circle" : "ellipse-outline"}
                size={24}
                color={isSelected ? colors.accent : colors.textTertiary}
              />
            </TouchableOpacity>
          );
        }}
        onEndReached={loadMore}
        onEndReachedThreshold={0.5}
        initialNumToRender={PAGE_SIZE}
        maxToRenderPerBatch={15}
        windowSize={5}
        removeClippedSubviews={true}
        ListFooterComponent={
          hasMore ? (
            <ActivityIndicator
              style={{ paddingVertical: 16 }}
              color={colors.accent}
            />
          ) : null
        }
      />

      <View
        style={[
          styles.bottomBar,
          { backgroundColor: colors.surface, borderTopColor: colors.border },
        ]}
      >
        <TouchableOpacity
          style={[
            styles.saveBtn,
            {
              backgroundColor: colors.accent,
              opacity: selectedExercises.length > 0 ? 1 : 0.5,
            },
          ]}
          onPress={handleSave}
          disabled={selectedExercises.length === 0}
        >
          <Text style={styles.saveBtnText}>
            Сохранить ({selectedExercises.length} упр.)
          </Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },

  label: { fontSize: 18, fontWeight: "700", padding: 16, paddingBottom: 8 },

  titleInput: {
    marginHorizontal: 16,
    height: 48,
    borderRadius: 10,
    paddingHorizontal: 14,
    fontSize: 16,
  },

  nextBtn: {
    margin: 16,
    paddingVertical: 14,
    borderRadius: 10,
    alignItems: "center",
  },

  nextBtnText: { color: "#fff", fontSize: 16, fontWeight: "700" },

  searchContainer: {
    flexDirection: "row",
    alignItems: "center",
    margin: 16,
    marginBottom: 8,

    paddingHorizontal: 12,
    borderRadius: 10,
    height: 44,
    gap: 8,
  },

  searchInput: { flex: 1, fontSize: 16 },

  filterRow: { paddingLeft: 16, marginBottom: 8, maxHeight: 40 },

  chip: {
    paddingHorizontal: 14,
    paddingVertical: 6,
    borderRadius: 16,
    marginRight: 8,
  },

  chipText: { fontSize: 13, fontWeight: "500" },

  exerciseRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingVertical: 12,

    borderBottomWidth: StyleSheet.hairlineWidth,
  },

  exerciseName: { fontSize: 15, fontWeight: "500" },

  exerciseMeta: { fontSize: 13, marginTop: 2 },

  bottomBar: {
    position: "absolute",
    bottom: 0,
    left: 0,
    right: 0,

    padding: 16,
    paddingBottom: 32,
    borderTopWidth: StyleSheet.hairlineWidth,
  },

  saveBtn: { paddingVertical: 14, borderRadius: 10, alignItems: "center" },

  saveBtnText: { color: "#fff", fontSize: 16, fontWeight: "700" },
});
