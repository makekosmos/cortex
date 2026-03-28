import { randomUUID } from "expo-crypto";
import type { Heading } from "@/types/task";

export function createHeading(
  title: string,
  projectId?: string | null,
): Heading {
  return {
    id: randomUUID(),
    title,
    sortOrder: 0,
    projectId: projectId ?? null,
  };
}
