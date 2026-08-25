export type FocusUpsertParams = {
  id?: string;
  name: string;
  domains: string[];
  icon: string;
  kind: string;
  preset?: boolean;
};

export function parseFocusDomains(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}
