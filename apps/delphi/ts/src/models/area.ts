import type { Area } from "@/types/task";

const uuid = () => crypto.randomUUID();

export function createArea(title: string): Area {
  return {
    id: uuid(),

    title,

    sortOrder: 0,

    isVisible: true,
  };
}
