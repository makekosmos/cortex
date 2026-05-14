import type { Heading } from "@/types/task";

const uuid = () => crypto.randomUUID();

export function createHeading(
  title: string,

  projectId?: string | null,
): Heading {
  return {
    id: uuid(),

    title,

    sortOrder: 0,

    projectId: projectId ?? null,
  };
}
