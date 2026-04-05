import { useState, useMemo, useEffect, useCallback } from "react";

import {
  View,
  Text,
  StyleSheet,
  FlatList,
  TextInput,
  TouchableOpacity,
  ScrollView,
  ActivityIndicator,
} from "react-native";

import { router } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { useThemeColor } from "@/lib/useThemeColor";

import { useWorkoutStore } from "@/lib/stores/workout-store";

import { useExercisesStore } from "@/lib/stores/exercises-store";

import { getRecentExerciseIds } from "@/lib/database";

import { MUSCLE_GROUPS, musclesRu, muscleRu } from "@/lib/types";

import React from "react";

const PAGE_SIZE = 30;

export default function AddExerciseScreen() {
  const colors = useThemeColor();

  const addExercise = useWorkoutStore((s) => s.addExercise);

  const exercises = useExercisesStore((s) => s.exercises);

  const [recentIds, setRecentIds] = useState<string[]>([]);

  const [search, setSearch] = useState("");

  const [selectedMuscle, setSelectedMuscle] = useState<string | null>(null);

  const [selected, setSelected] = useState<Set<string>>(new Set());

  const [visibleCount, setVisibleCount] = useState(PAGE_SIZE);

  useEffect(() => {
    getRecentExerciseIds().then(setRecentIds);
  }, []);

  useEffect(() => {
    setVisibleCount(PAGE_SIZE);
  }, [search, selectedMuscle]);

  const filtered = useMemo(() => {
    let result = exercises;

    if (search) {
      const q = search.toLowerCase();

      result = result.filter((e) => e.name.toLowerCase().includes(q));
    }

    if (selectedMuscle) {
      result = result.filter((e) => e.primary_muscles.includes(selectedMuscle));
    }

    if (!search && !selectedMuscle && recentIds.length > 0) {
      const recentSet = new Set(recentIds);

      result = [
        ...result.filter((e) => recentSet.has(e.id)),

        ...result.filter((e) => !recentSet.has(e.id)),
      ];
    }

    return result;
  }, [exercises, search, selectedMuscle, recentIds]);

  const visible = useMemo(
    () => filtered.slice(0, visibleCount),
    [filtered, visibleCount],
  );

  const hasMore = visibleCount < filtered.length;

  const loadMore = useCallback(() => {
    if (hasMore) setVisibleCount((prev) => prev + PAGE_SIZE);
  }, [hasMore]);

  function toggleSelect(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);

      if (next.has(id)) next.delete(id);
      else next.add(id);

      return next;
    });
  }

  async function handleAdd() {
    const toAdd = exercises.filter((e) => selected.has(e.id));

    for (const ex of toAdd) await addExercise(ex);

    router.back();
  }

  const renderItem = useCallback(
    ({ item }: { item: any }) => {
      const isSelected = selected.has(item.id);

      return (
        <TouchableOpacity
          style={[styles.exerciseRow, { borderBottomColor: colors.border }]}
          onPress={() => toggleSelect(item.id)}
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
    },
    [colors, selected],
  );

  return (
    <View style={[styles.container, { backgroundColor: colors.background }]}>
      <View
        style={[styles.searchContainer, { backgroundColor: colors.surface }]}
      >
        <Ionicons name="search" size={18} color={colors.textTertiary} />
        <TextInput
          style={[styles.searchInput, { color: colors.text }]}
          placeholder="Поиск упражнений..."
          placeholderTextColor={colors.textTertiary}
          value={search}
          onChangeText={setSearch}
          autoFocus
          autoCorrect={false}
        />
        {search.length > 0 && (
          <TouchableOpacity onPress={() => setSearch("")}>
            <Ionicons
              name="close-circle"
              size={18}
              color={colors.textTertiary}
            />
          </TouchableOpacity>
        )}
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

      <View style={styles.createRow}>
        <TouchableOpacity
          style={[styles.createBtn, { backgroundColor: colors.surface }]}
          onPress={() => router.push("/exercise/create")}
          activeOpacity={0.7}
        >
          <Ionicons name="add-circle-outline" size={18} color={colors.accent} />
          <Text style={[styles.createBtnText, { color: colors.accent }]}>
            Создать своё упражнение
          </Text>
        </TouchableOpacity>
      </View>

      <FlatList
        data={visible}
        keyExtractor={(item) => item.id}
        contentContainerStyle={{ paddingHorizontal: 16, paddingBottom: 100 }}
        renderItem={renderItem}
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

      {selected.size > 0 && (
        <View
          style={[
            styles.bottomBar,
            { backgroundColor: colors.surface, borderTopColor: colors.border },
          ]}
        >
          <TouchableOpacity
            style={[styles.addBtn, { backgroundColor: colors.accent }]}
            onPress={handleAdd}
          >
            <Text style={styles.addBtnText}>Добавить ({selected.size})</Text>
          </TouchableOpacity>
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },

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

  createRow: { paddingHorizontal: 16, marginBottom: 8 },

  createBtn: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",

    paddingVertical: 10,
    borderRadius: 10,
    gap: 6,
  },

  createBtnText: { fontSize: 14, fontWeight: "600" },

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

  addBtn: { paddingVertical: 14, borderRadius: 10, alignItems: "center" },

  addBtnText: { color: "#fff", fontSize: 16, fontWeight: "700" },
});
