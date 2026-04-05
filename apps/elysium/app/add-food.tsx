import React, { useState } from "react";

import { StyleSheet } from "react-native";

import { useLocalSearchParams, useRouter } from "expo-router";

import { SafeAreaView } from "react-native-safe-area-context";

import { FoodSearch } from "@/components/FoodSearch";

import { QuantityPicker } from "@/components/QuantityPicker";

import { useNutritionStore } from "@/stores/nutrition-store";

import { useFoodStore } from "@/stores/food-store";

import { colors } from "@/theme";

import type { FoodItem, MealType } from "@/types/nutrition";

type Step = "search" | "quantity";

export default function AddFoodScreen() {
  const router = useRouter();

  const { mealType } = useLocalSearchParams<{ mealType: MealType }>();

  const addEntry = useNutritionStore((s) => s.addEntry);

  const markUsed = useFoodStore((s) => s.markUsed);

  const [step, setStep] = useState<Step>("search");

  const [selectedFood, setSelectedFood] = useState<FoodItem | null>(null);

  const meal = mealType ?? "snack";

  return (
    <SafeAreaView style={styles.safe} edges={["top"]}>
      {step === "search" && (
        <FoodSearch
          onSelect={(food) => {
            setSelectedFood(food);
            setStep("quantity");
          }}
          onCreateNew={() => router.push("/create-food")}
          mealType={meal}
        />
      )}
      {step === "quantity" && selectedFood && (
        <QuantityPicker
          food={selectedFood}
          mealType={meal}
          onConfirm={(qty, selectedMeal) => {
            markUsed(selectedFood);
            addEntry(selectedFood, qty, selectedMeal);
            router.back();
          }}
          onBack={() => setStep("search")}
        />
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safe: { flex: 1, backgroundColor: colors.bg.primary },
});
