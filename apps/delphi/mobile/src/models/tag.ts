import { randomUUID } from "expo-crypto";
import type { Tag } from "@/types/task";

export function createTag(
  title: string,
  color: string = "blue",
  shortcut?: string | null,
): Tag {
  return {
    id: randomUUID(),
    title,
    color,
    shortcut: shortcut ?? null,
  };
}
