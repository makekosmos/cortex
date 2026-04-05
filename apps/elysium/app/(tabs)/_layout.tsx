import { Tabs } from "expo-router";

import { FloatingTabBar } from "@/components/FloatingTabBar";

export default function TabLayout() {
  return (
    <>
      <Tabs
        screenOptions={{
          headerShown: false,

          tabBarStyle: { display: "none" },
        }}
      >
        <Tabs.Screen name="index" />
        <Tabs.Screen name="water" />
        <Tabs.Screen name="profile" />
      </Tabs>
      <FloatingTabBar />
    </>
  );
}
