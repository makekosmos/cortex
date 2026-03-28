import { randomUUID } from "expo-crypto";
import type { Area } from "@/types/task";

export function createArea(title: string): Area {
  return {
    id: randomUUID(),
    title,
    sortOrder: 0,
    isVisible: true,
  };
}
