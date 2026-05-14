import { colors } from '../tokens/colors';

export type ThemeMode = 'light' | 'dark';

export type ColorToken = keyof typeof colors.light;

export function getColor(mode: ThemeMode, token: ColorToken): string {
  return colors[mode][token];
}

export { colors } from '../tokens/colors';
