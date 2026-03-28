import React from "react";
import { SmartList } from "@/types/task";
import SmartListScreen from "@/components/SmartListScreen";

export default function UpcomingScreen() {
  return <SmartListScreen list={SmartList.Upcoming} />;
}
