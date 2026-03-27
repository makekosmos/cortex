import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { StyleSheet, ActivityIndicator, View } from 'react-native';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import { useEffect } from 'react';
import { colors } from '@/theme';
import { useNutritionStore } from '@/stores/nutrition-store';
import { useWaterStore } from '@/stores/water-store';
import { useFoodStore } from '@/stores/food-store';
import { useSettingsStore } from '@/stores/settings-store';
import { useSyncStore } from '@/sync/sync-store';

SplashScreen.preventAutoHideAsync();

const queryClient = new QueryClient();

export default function RootLayout() {
  const [fontsLoaded] = useFonts({
    'FeatureDisplay': require('../assets/fonts/FeatureDisplay-Regular.ttf'),
    'Geist': require('../assets/fonts/Geist_400Regular.ttf'),
    'Geist-SemiBold': require('../assets/fonts/Geist_600SemiBold.ttf'),
    'Geist-Bold': require('../assets/fonts/Geist_700Bold.ttf'),
    'GeistMono': require('../assets/fonts/GeistMono_400Regular.ttf'),
    'GeistMono-Bold': require('../assets/fonts/GeistMono_700Bold.ttf'),
  });

  useEffect(() => {
    if (fontsLoaded) {
      // Hydrate all stores from SQLite before showing app
      useNutritionStore.getState().hydrate();
      useWaterStore.getState().hydrate();
      useFoodStore.getState().hydrate();
      useSettingsStore.getState().hydrate();
      useSyncStore.getState().hydrate(); // auto-connects if previously paired
      SplashScreen.hideAsync();
    }
  }, [fontsLoaded]);

  if (!fontsLoaded) {
    return (
      <View style={[styles.root, styles.center]}>
        <ActivityIndicator color={colors.accent} />
      </View>
    );
  }

  return (
    <QueryClientProvider client={queryClient}>
      <GestureHandlerRootView style={styles.root}>
        <StatusBar style="light" />
        <Stack
          screenOptions={{
            headerShown: false,
            contentStyle: { backgroundColor: colors.bg.primary },
            animation: 'slide_from_right',
          }}
        />
      </GestureHandlerRootView>
    </QueryClientProvider>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: colors.bg.primary,
  },
  center: {
    justifyContent: 'center',
    alignItems: 'center',
  },
});
