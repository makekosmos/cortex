import React, { useState, useRef } from 'react';
import { View, StyleSheet, Text, Pressable, ActivityIndicator } from 'react-native';
import { useRouter, useLocalSearchParams } from 'expo-router';
import { SafeAreaView } from 'react-native-safe-area-context';
import { CameraView, useCameraPermissions, type BarcodeScanningResult } from 'expo-camera';
import { Ionicons } from '@expo/vector-icons';
import { colors, fontSize, spacing, radius, fonts } from '@/theme';
import { lookupBarcode } from '@/api/open-food-facts';
import { searchFatSecret } from '@/api/fatsecret';
import { useFoodStore } from '@/stores/food-store';
import { useNutritionStore } from '@/stores/nutrition-store';
import { QuantityPicker } from '@/components/QuantityPicker';
import type { FoodItem, MealType } from '@/types/nutrition';

export default function ScannerScreen() {
  const router = useRouter();
  const { mealType } = useLocalSearchParams<{ mealType: string }>();
  const meal = (mealType ?? 'snack') as MealType;
  const addCustomFood = useFoodStore((s) => s.addCustomFood);
  const markUsed = useFoodStore((s) => s.markUsed);
  const addEntry = useNutritionStore((s) => s.addEntry);
  const [permission, requestPermission] = useCameraPermissions();
  const [scanning, setScanning] = useState(true);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<FoodItem | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scannedCode, setScannedCode] = useState<string | null>(null);
  const lastScannedRef = useRef<string>('');

  const handleBarCodeScanned = async (scanResult: BarcodeScanningResult) => {
    const code = scanResult.data;
    if (!scanning || loading || code === lastScannedRef.current) return;

    lastScannedRef.current = code;
    setScanning(false);
    setLoading(true);
    setError(null);
    setScannedCode(code);

    // Try OFF first (has barcode lookup)
    let food = await lookupBarcode(code);

    // Fallback: search FatSecret by barcode number
    if (!food) {
      const fsResults = await searchFatSecret(code);
      if (fsResults.length > 0) food = fsResults[0];
    }

    if (food) {
      setResult(food);
    } else {
      setError(`Продукт не найден (${code})`);
    }
    setLoading(false);
  };

  const handleConfirm = (qty: number, selectedMeal: MealType) => {
    if (result) {
      addCustomFood({
        name: result.name,
        brand: result.brand,
        servingSize: result.servingSize,
        servingUnit: result.servingUnit,
        macros: result.macros,
      });
      markUsed(result);
      addEntry(result, qty, selectedMeal);
      router.dismiss(2);
    }
  };

  const handleRetry = () => {
    lastScannedRef.current = '';
    setResult(null);
    setError(null);
    setScannedCode(null);
    setScanning(true);
  };

  const handleCreateFood = () => {
    router.replace('/create-food');
  };

  if (!permission) {
    return (
      <View style={styles.centered}>
        <ActivityIndicator color={colors.accent} />
      </View>
    );
  }

  if (!permission.granted) {
    return (
      <SafeAreaView style={styles.safe} edges={['top']}>
        <View style={styles.centered}>
          <Ionicons name="camera-outline" size={64} color={colors.text.muted} />
          <Text style={styles.permText}>Нужен доступ к камере для сканирования штрих-кодов</Text>
          <Pressable style={styles.permBtn} onPress={requestPermission}>
            <Text style={styles.permBtnText}>Разрешить камеру</Text>
          </Pressable>
          <Pressable style={styles.backLink} onPress={() => router.back()}>
            <Text style={styles.backLinkText}>Назад</Text>
          </Pressable>
        </View>
      </SafeAreaView>
    );
  }

  if (result) {
    return (
      <SafeAreaView style={styles.safe} edges={['top']}>
        <QuantityPicker
          food={result}
          mealType={meal}
          onConfirm={handleConfirm}
          onBack={handleRetry}
        />
      </SafeAreaView>
    );
  }

  return (
    <View style={styles.container}>
      <CameraView
        style={StyleSheet.absoluteFill}
        facing="back"
        barcodeScannerSettings={{
          barcodeTypes: ['ean13', 'ean8', 'upc_a', 'upc_e'],
        }}
        onBarcodeScanned={scanning ? handleBarCodeScanned : undefined}
      />

      {/* Overlay */}
      <SafeAreaView style={styles.overlay} edges={['top']}>
        <View style={styles.topBar}>
          <Pressable onPress={() => router.back()} hitSlop={16}>
            <Ionicons name="close" size={28} color="#fff" />
          </Pressable>
          <Text style={styles.topTitle}>Сканер штрих-кода</Text>
          <View style={{ width: 28 }} />
        </View>

        {/* Viewfinder */}
        <View style={styles.viewfinderWrap}>
          <View style={styles.viewfinder}>
            <View style={[styles.corner, styles.topLeft]} />
            <View style={[styles.corner, styles.topRight]} />
            <View style={[styles.corner, styles.bottomLeft]} />
            <View style={[styles.corner, styles.bottomRight]} />
          </View>
          {scanning && !loading && (
            <Text style={styles.hint}>Наведите камеру на штрих-код</Text>
          )}
        </View>

        {/* Bottom panel */}
        <View style={styles.bottomPanel}>
          {loading && (
            <View style={styles.statusCard}>
              <ActivityIndicator color={colors.accent} />
              <Text style={styles.statusText}>Ищем продукт...</Text>
            </View>
          )}

          {error && (
            <View style={styles.statusCard}>
              <Ionicons name="alert-circle" size={24} color={'#FFB347'} />
              <Text style={styles.statusText}>{error}</Text>
              <View style={styles.errorActions}>
                <Pressable style={styles.retryBtn} onPress={handleRetry}>
                  <Ionicons name="scan-outline" size={18} color={colors.accent} />
                  <Text style={styles.retryText}>Сканировать ещё</Text>
                </Pressable>
                <Pressable style={styles.createBtn} onPress={handleCreateFood}>
                  <Ionicons name="add-circle-outline" size={18} color={colors.accent} />
                  <Text style={styles.retryText}>Создать продукт</Text>
                </Pressable>
              </View>
            </View>
          )}
        </View>
      </SafeAreaView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  safe: {
    flex: 1,
    backgroundColor: colors.bg.primary,
  },
  centered: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    backgroundColor: colors.bg.primary,
    paddingHorizontal: spacing.xl,
  },
  permText: {
    fontSize: fontSize.md,
    color: colors.text.secondary,
    textAlign: 'center',
    marginTop: spacing.lg,
    marginBottom: spacing.xl,
  },
  permBtn: {
    backgroundColor: colors.accent,
    borderRadius: radius.md,
    paddingVertical: spacing.md,
    paddingHorizontal: spacing.xl,
  },
  permBtnText: {
    fontSize: fontSize.md,
    fontFamily: fonts.semiBold,
    color: colors.text.inverse,
  },
  backLink: {
    marginTop: spacing.lg,
  },
  backLinkText: {
    fontSize: fontSize.sm,
    color: colors.text.muted,
  },
  overlay: {
    flex: 1,
    justifyContent: 'space-between',
  },
  topBar: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
  },
  topTitle: {
    fontSize: fontSize.md,
    fontFamily: fonts.semiBold,
    color: '#fff',
  },
  viewfinderWrap: {
    alignItems: 'center',
  },
  viewfinder: {
    width: 260,
    height: 160,
    position: 'relative',
  },
  corner: {
    position: 'absolute',
    width: 30,
    height: 30,
    borderColor: colors.accent,
  },
  topLeft: {
    top: 0,
    left: 0,
    borderTopWidth: 3,
    borderLeftWidth: 3,
    borderTopLeftRadius: 8,
  },
  topRight: {
    top: 0,
    right: 0,
    borderTopWidth: 3,
    borderRightWidth: 3,
    borderTopRightRadius: 8,
  },
  bottomLeft: {
    bottom: 0,
    left: 0,
    borderBottomWidth: 3,
    borderLeftWidth: 3,
    borderBottomLeftRadius: 8,
  },
  bottomRight: {
    bottom: 0,
    right: 0,
    borderBottomWidth: 3,
    borderRightWidth: 3,
    borderBottomRightRadius: 8,
  },
  hint: {
    fontSize: fontSize.sm,
    color: '#ffffffCC',
    marginTop: spacing.lg,
  },
  bottomPanel: {
    paddingHorizontal: spacing.lg,
    paddingBottom: spacing.xl,
  },
  statusCard: {
    backgroundColor: colors.bg.card + 'EE',
    borderRadius: radius.lg,
    padding: spacing.lg,
    alignItems: 'center',
    gap: spacing.sm,
  },
  statusText: {
    fontSize: fontSize.md,
    color: colors.text.primary,
    textAlign: 'center',
  },
  errorActions: {
    flexDirection: 'row',
    gap: spacing.md,
    marginTop: spacing.sm,
  },
  retryBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
  },
  createBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
  },
  retryText: {
    fontSize: fontSize.sm,
    color: colors.accent,
    fontFamily: fonts.semiBold,
  },
});
