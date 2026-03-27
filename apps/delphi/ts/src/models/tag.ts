import type { Tag } from '@/types/task';
const uuid = () => crypto.randomUUID();

export function createTag(
  title: string,
  color: string = 'blue',
  shortcut?: string | null,
): Tag {
  return {
    id: uuid(),
    title,
    color,
    shortcut: shortcut ?? null,
  };
}
