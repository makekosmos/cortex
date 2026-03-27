import { Moon, Sun } from 'lucide-react';
import { useTheme } from '@/features/themeProvider';

export function ModeToggle() {
  const { theme, toggleTheme } = useTheme();
  return (
    <div className="inline-flex items-center gap-2 rounded-xl p-1 hover:bg-(--secondary)">
      <button
        type="button"
        onClick={toggleTheme}
        className="rounded-md p-3 text-sm"
      >
        {theme === 'dark' ? <Moon /> : <Sun />}
      </button>
      {/* <button onClick={() => setTheme('system')} className="text-xs underline">
        system
      </button> */}
    </div>
  );
}
