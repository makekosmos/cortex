import Colors from '../constants/Colors';
import { useSettingsStore } from './stores/settings-store';

export function useThemeColor() {
  const theme = useSettingsStore((s) => s.theme);
  return Colors[theme];
}
