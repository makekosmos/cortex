import React from "react";
import { SmartList } from "@/types/task";
import SmartListScreen from "@/components/SmartListScreen";

export default function TodayScreen() {
  return <SmartListScreen list={SmartList.Today} />;
}
