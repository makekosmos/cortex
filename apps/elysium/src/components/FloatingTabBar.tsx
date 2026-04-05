import React, { useEffect, useRef } from "react";

import { View, StyleSheet, Pressable, Animated } from "react-native";

import { useRouter, usePathname } from "expo-router";

import { useSafeAreaInsets } from "react-native-safe-area-context";

import { BlurView } from "expo-blur";

import { LinearGradient } from "expo-linear-gradient";

import { Ionicons } from "@expo/vector-icons";

const TABS = [
  {
    route: "/(tabs)",
    icon: "sunny-outline" as const,
    iconActive: "sunny" as const,
  },

  {
    route: "/(tabs)/water",
    icon: "water-outline" as const,
    iconActive: "water" as const,
  },

  {
    route: "/(tabs)/profile",
    icon: "person-outline" as const,
    iconActive: "person" as const,
  },
] as const;

const INNER_PAD = 6;

const INDICATOR_H = 50;

const INDICATOR_W = 60;

const TAB_W = INDICATOR_W;

const GAP = 0;

const CAPSULE_H = INDICATOR_H + INNER_PAD * 2;

const PAD_H = INNER_PAD;

const GLASS_BG = "rgba(18, 18, 26, 0.7)";

const GLASS_BORDER = "rgba(255,255,255,0.18)";

const INDICATOR_BG = "rgba(45, 45, 58, 1)";

const INDICATOR_BORDER = "rgba(255,255,255,0.18)";

const FADE_HEIGHT = 30;

function getActiveIndex(pathname: string): number {
  if (pathname === "/water" || pathname === "/(tabs)/water") return 1;

  if (pathname === "/profile" || pathname === "/(tabs)/profile") return 2;

  return 0;
}

export function FloatingTabBar() {
  const router = useRouter();

  const pathname = usePathname();

  const insets = useSafeAreaInsets();

  const activeIndex = getActiveIndex(pathname);

  const indicatorOffset = (idx: number) => idx * (TAB_W + GAP);

  const indicatorAnim = useRef(
    new Animated.Value(indicatorOffset(activeIndex)),
  ).current;

  const scaleAnims = useRef(TABS.map(() => new Animated.Value(1))).current;

  useEffect(() => {
    Animated.spring(indicatorAnim, {
      toValue: indicatorOffset(activeIndex),

      damping: 18,

      stiffness: 200,

      mass: 0.8,

      useNativeDriver: true,
    }).start();

    scaleAnims.forEach((anim, i) => {
      Animated.spring(anim, {
        toValue: i === activeIndex ? 1.1 : 1,

        damping: 14,

        stiffness: 180,

        useNativeDriver: true,
      }).start();
    });
  }, [activeIndex]);

  const CAPSULE_W =
    PAD_H + TABS.length * TAB_W + (TABS.length - 1) * GAP + PAD_H;

  const bottomOffset = Math.max(insets.bottom, 16);

  return (
    <View style={styles.wrapper} pointerEvents="box-none">
      {/* Fade gradient */}
      <LinearGradient
        colors={["transparent", "rgba(10,10,15,0.85)", "rgba(10,10,15,1)"]}
        style={[
          styles.fade,
          { height: FADE_HEIGHT + CAPSULE_H + bottomOffset },
        ]}
        pointerEvents="none"
      />

      {/* Tab bar */}
      <View style={[styles.barWrap, { bottom: bottomOffset }]}>
        <View
          style={[
            styles.pill,
            {
              width: CAPSULE_W,
              height: CAPSULE_H,
              borderRadius: CAPSULE_H / 2,
            },
          ]}
        >
          <BlurView
            intensity={50}
            tint="dark"
            style={StyleSheet.absoluteFill}
          />
          <View style={styles.glassFill} />
          <View style={[styles.glassBorder, { borderRadius: CAPSULE_H / 2 }]} />

          <View style={styles.row}>
            <Animated.View
              style={[
                styles.indicator,

                { transform: [{ translateX: indicatorAnim }] },
              ]}
            />

            {TABS.map((tab, i) => {
              const isActive = i === activeIndex;

              return (
                <Pressable
                  key={tab.route}
                  style={styles.tabHit}
                  onPress={() => router.push(tab.route as any)}
                >
                  <Animated.View
                    style={{ transform: [{ scale: scaleAnims[i] }] }}
                  >
                    <Ionicons
                      name={isActive ? tab.iconActive : tab.icon}
                      size={22}
                      color={isActive ? "#FFF" : "rgba(255,255,255,0.4)"}
                    />
                  </Animated.View>
                </Pressable>
              );
            })}
          </View>
        </View>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  wrapper: {
    position: "absolute",

    left: 0,

    right: 0,

    bottom: 0,
  },

  fade: {
    position: "absolute",

    left: 0,

    right: 0,

    bottom: 0,
  },

  barWrap: {
    position: "absolute",

    left: 0,

    right: 0,

    alignItems: "center",
  },

  pill: {
    overflow: "hidden",

    shadowColor: "#000",

    shadowOffset: { width: 0, height: 8 },

    shadowOpacity: 0.45,

    shadowRadius: 24,

    elevation: 20,
  },

  glassFill: {
    ...StyleSheet.absoluteFillObject,

    backgroundColor: GLASS_BG,
  },

  glassBorder: {
    ...StyleSheet.absoluteFillObject,

    borderWidth: 1,

    borderColor: GLASS_BORDER,
  },

  row: {
    flex: 1,

    flexDirection: "row",

    alignItems: "center",

    paddingHorizontal: PAD_H,
  },

  indicator: {
    position: "absolute",

    left: PAD_H,

    top: INNER_PAD,

    width: INDICATOR_W,

    height: INDICATOR_H,

    borderRadius: INDICATOR_H / 2,

    backgroundColor: INDICATOR_BG,

    borderWidth: 1,

    borderColor: INDICATOR_BORDER,
  },

  tabHit: {
    width: TAB_W,

    height: CAPSULE_H,

    justifyContent: "center",

    alignItems: "center",

    marginRight: GAP,
  },
});
