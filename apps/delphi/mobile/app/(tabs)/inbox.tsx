import React from "react";
import { SmartList } from "@/types/task";
import SmartListScreen from "@/components/SmartListScreen";

export default function InboxScreen() {
  return <SmartListScreen list={SmartList.Inbox} />;
}
