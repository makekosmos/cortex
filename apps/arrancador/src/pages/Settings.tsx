import {
  Check,
  ExternalLink,
  FolderOpen,
  HardDrive,
  Key,
  Loader2,
  Monitor,
  Moon,
  Power,
  RefreshCw,
  Shield,
  Sun,
  Sparkles,
} from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";

import { useTheme } from "@/components/theme-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { useSettingsState } from "@/hooks/useSettingsState";
import { cn } from "@/lib/utils";

type SettingsSection = "all" | "appearance" | "system" | "backup" | "sqoba" | "rawg";

export default function Settings() {
  const { theme, setTheme } = useTheme();
  const {
    loading,
    saving,
    backupDirectory,
    setBackupDirectory,
    autoBackup,
    setAutoBackup,
    backupBeforeLaunch,
    setBackupBeforeLaunch,
    compressionEnabled,
    handleCompressionToggle,
    compressionLevel,
    handleCompressionLevelChange,
    skipCompressionOnce,
    setSkipCompressionOnce,
    maxBackups,
    handleMaxBackupsChange,
    rawgApiKey,
    setRawgApiKey,
    autoStart,
    toggleAutoStart,
    saveSettings,
    selectBackupDirectory,
    refreshSqobaManifest,
  } = useSettingsState();
  const [manifestRefreshing, setManifestRefreshing] = useState(false);
  const [manifestStatus, setManifestStatus] = useState<string | null>(null);
  const [activeSection, setActiveSection] = useState<SettingsSection>("all");

  const sectionItems: Array<{
    id: SettingsSection;
    label: string;
    icon: "monitor" | "power" | "shield" | "sparkles" | "key";
  }> = [
    { id: "all", label: "Все", icon: "monitor" },
    { id: "appearance", label: "Внешний вид", icon: "monitor" },
    { id: "system", label: "Система", icon: "power" },
    { id: "backup", label: "Резервное копирование", icon: "shield" },
    { id: "sqoba", label: "SQOBA", icon: "sparkles" },
    { id: "rawg", label: "RAWG", icon: "key" },
  ];

  const handleRefreshManifest = async () => {
    if (manifestRefreshing) return;
    setManifestRefreshing(true);
    setManifestStatus(null);
    try {
      await refreshSqobaManifest();
      setManifestStatus("Манифест обновлён");
    } catch (error) {
      console.error("Failed to refresh SQOBA manifest:", error);
      setManifestStatus("Не удалось обновить манифест");
    } finally {
      setManifestRefreshing(false);
    }
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <Loader2 className="h-8 w-8 animate-spin" />
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl px-4 py-4 sm:px-6 sm:py-6">
      <div className="mb-6 sm:mb-8">
        <h1 className="text-xl font-bold tracking-tight sm:text-2xl">
          Настройки
        </h1>
        <p className="text-xs text-muted-foreground sm:text-sm">
          Настройте параметры лаунчера
        </p>
      </div>

      <div className="mb-6 flex flex-wrap gap-2">
        {sectionItems.map((item) => {
          const iconClass = cn(
            "h-4 w-4 transition-colors",
            activeSection === item.id && "text-primary-foreground",
          );

          return (
            <button
              key={item.id}
              type="button"
              aria-pressed={activeSection === item.id}
              onClick={() => {
                setActiveSection(item.id);
                if (item.id === "all") return;
                const section = document.getElementById(`settings-${item.id}`);
                section?.scrollIntoView({ behavior: "smooth", block: "start" });
              }}
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition-colors",
                activeSection === item.id
                  ? "border-primary bg-primary text-primary-foreground shadow-sm"
                  : "border-border/70 bg-card/80 text-card-foreground hover:bg-accent/80",
              )}
            >
              {item.icon === "monitor" && <Monitor className={iconClass} />}
              {item.icon === "power" && <Power className={iconClass} />}
              {item.icon === "shield" && <Shield className={iconClass} />}
              {item.icon === "sparkles" && <Sparkles className={iconClass} />}
              {item.icon === "key" && <Key className={iconClass} />}
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>

      <div className="space-y-6 pb-20 sm:space-y-8 sm:pb-0">
        <section id="settings-appearance" className="space-y-4">
          <div className="flex items-center gap-2">
            <Monitor className="h-5 w-5" />
            <h2 className="text-base font-semibold sm:text-lg">Внешний вид</h2>
          </div>

          <div className="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
            <div className="mb-3 text-sm font-medium">Тема</div>
            <div className="flex flex-wrap gap-2">
              <Button
                variant={theme === "light" ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme("light")}
                className="flex-1 gap-2 sm:flex-none"
              >
                <Sun className="h-4 w-4" />
                Светлая
              </Button>
              <Button
                variant={theme === "dark" ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme("dark")}
                className="flex-1 gap-2 sm:flex-none"
              >
                <Moon className="h-4 w-4" />
                Тёмная
              </Button>
              <Button
                variant={theme === "system" ? "default" : "outline"}
                size="sm"
                onClick={() => setTheme("system")}
                className="w-full gap-2 sm:w-auto"
              >
                <Monitor className="h-4 w-4" />
                Системная
              </Button>
            </div>
          </div>
        </section>

        <section id="settings-system" className="space-y-4">
          <div className="flex items-center gap-2">
            <Power className="h-5 w-5" />
            <h2 className="text-lg font-semibold">Система</h2>
          </div>

          <div className="overflow-hidden rounded-2xl border border-border/70 bg-card/90 shadow-sm">
            <div className="flex items-center justify-between gap-4 p-4">
              <button type="button" className="flex-1 text-left" onClick={() => toggleAutoStart()}>
                <span id="setting-autostart" className="block text-sm font-medium">
                  Автозапуск
                </span>
                <span className="text-xs text-muted-foreground">
                  Запускать приложение при старте системы для фонового трекинга игр
                </span>
              </button>
              <Switch
                checked={autoStart}
                onCheckedChange={toggleAutoStart}
                aria-labelledby="setting-autostart"
              />
            </div>
          </div>
        </section>

        <section id="settings-backup" className="space-y-4">
          <div className="flex items-center gap-2">
            <Shield className="h-5 w-5" />
            <h2 className="text-lg font-semibold">Резервное копирование</h2>
          </div>

          <div className="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
            <div>
              <div className="mb-2 flex items-center justify-between gap-3">
                <div className="text-sm font-medium">Движок бэкапов</div>
                <div className="flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-600 dark:text-emerald-400">
                  <Check className="h-3 w-3" />
                  Встроенный (Native)
                </div>
              </div>
              <p className="text-xs text-muted-foreground">
                Используется встроенный движок, совместимый с манифестом Ludusavi.
              </p>
            </div>

            <div className="rounded-xl border border-dashed border-border/70 p-3">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <div className="text-sm font-medium">Манифест SQOBA</div>
                  <div className="text-xs text-muted-foreground">
                    Нужен для автопоиска сохранений (Ludusavi/PCGamingWiki).
                  </div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleRefreshManifest}
                  disabled={manifestRefreshing}
                  className="gap-2"
                >
                  {manifestRefreshing ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : (
                    <RefreshCw className="h-3 w-3" />
                  )}
                  Обновить
                </Button>
              </div>
              {manifestStatus ? (
                <div className="mt-2 text-xs text-muted-foreground">{manifestStatus}</div>
              ) : null}
            </div>

            <div>
              <label
                className="mb-2 block text-sm font-medium"
                htmlFor="backup-directory"
              >
                Папка для бэкапов
              </label>
              <div className="flex gap-2">
                <Input
                  id="backup-directory"
                  value={backupDirectory}
                  onChange={(event) => setBackupDirectory(event.target.value)}
                  placeholder="Путь к папке"
                  className="flex-1"
                />
                <Button
                  variant="outline"
                  type="button"
                  onClick={selectBackupDirectory}
                  aria-label="Выбрать папку для бэкапов"
                >
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
            </div>

            <div>
              <label className="mb-2 block text-sm font-medium" htmlFor="max-backups">
                Макс. количество бэкапов на игру
              </label>
              <Input
                id="max-backups"
                type="number"
                min={1}
                max={100}
                value={maxBackups}
                onChange={(event) =>
                  handleMaxBackupsChange(parseInt(event.target.value, 10))
                }
                className="w-24"
              />
              <p className="mt-1 text-xs text-muted-foreground">
                Старые копии будут удалены при превышении лимита
              </p>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between gap-3 rounded-xl px-2 py-2 hover:bg-accent/50 transition-colors">
                <button
                  type="button"
                  className="flex-1 cursor-pointer text-left"
                  onClick={() => setAutoBackup((prev) => !prev)}
                >
                  <span id="setting-auto-backup" className="text-sm">
                    Включить автоматические бэкапы
                  </span>
                </button>
                <Switch
                  checked={autoBackup}
                  onCheckedChange={setAutoBackup}
                  aria-labelledby="setting-auto-backup"
                />
              </div>

              <div className="flex items-center justify-between gap-3 rounded-xl px-2 py-2 hover:bg-accent/50 transition-colors">
                <button
                  type="button"
                  className="flex-1 cursor-pointer text-left"
                  onClick={() => setBackupBeforeLaunch((prev) => !prev)}
                >
                  <span id="setting-backup-before-launch" className="text-sm">
                    Предлагать создать бэкап перед запуском
                  </span>
                </button>
                <Switch
                  checked={backupBeforeLaunch}
                  onCheckedChange={setBackupBeforeLaunch}
                  aria-labelledby="setting-backup-before-launch"
                />
              </div>
            </div>
          </div>
        </section>

        <section id="settings-sqoba" className="space-y-4">
          <div className="flex items-center gap-2">
            <Sparkles className="h-5 w-5" />
            <h2 className="text-lg font-semibold">SQOBA</h2>
          </div>

          <div className="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3 rounded-xl bg-background/40 px-3 py-3">
              <div>
                <div className="text-sm font-medium">SQOBA менеджер</div>
                <div className="text-xs text-muted-foreground">
                  Быстрый доступ к управлению бэкапами и сохранениями через старую страницу SQOBA.
                </div>
              </div>
              <Button asChild variant="outline" size="sm" className="gap-2">
                <Link to="/sqoba">
                  <Sparkles className="h-4 w-4" />
                  Открыть SQOBA
                </Link>
              </Button>
            </div>

            <div className="rounded-xl border border-border/70 p-3">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <div className="text-sm font-medium">Манифест SQOBA</div>
                  <div className="text-xs text-muted-foreground">
                    Обновить локальный список игр и названий сохранений.
                  </div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleRefreshManifest}
                  disabled={manifestRefreshing}
                  className="gap-2"
                >
                  {manifestRefreshing ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : (
                    <RefreshCw className="h-3 w-3" />
                  )}
                  Обновить
                </Button>
              </div>
              {manifestStatus ? (
                <div className="mt-2 text-xs text-muted-foreground">{manifestStatus}</div>
              ) : null}
            </div>
          </div>
        </section>

        <section id="settings-compression" className="space-y-4">
          <div className="flex items-center gap-2">
            <HardDrive className="h-5 w-5" />
            <h2 className="text-lg font-semibold">Сжатие SQOBA</h2>
          </div>

          <div className="space-y-4 rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
            <div className="flex items-center justify-between gap-3 rounded-xl px-2 py-2 hover:bg-accent/50 transition-colors">
              <button
                type="button"
                className="flex-1 cursor-pointer text-left"
                onClick={() => handleCompressionToggle(!compressionEnabled)}
              >
                <div>
                  <span id="setting-compression" className="text-sm font-medium">
                    Включить сжатие
                  </span>
                  <span className="block text-xs text-muted-foreground">
                    Сжатые бэкапы занимают меньше места и лучше хранят историю.
                  </span>
                </div>
              </button>
              <Switch
                checked={compressionEnabled}
                onCheckedChange={handleCompressionToggle}
                aria-labelledby="setting-compression"
              />
            </div>

            <div className={cn("space-y-3", !compressionEnabled && "opacity-50")}>
              <div className="flex items-center gap-3">
                <label
                  className="text-xs text-muted-foreground"
                  htmlFor="compression-level"
                >
                  Уровень
                </label>
                <Input
                  id="compression-level"
                  type="number"
                  min={1}
                  max={100}
                  value={compressionLevel}
                  onChange={(event) =>
                    handleCompressionLevelChange(parseInt(event.target.value, 10))
                  }
                  className="w-20"
                  disabled={!compressionEnabled}
                />
                <div className="ml-auto text-xs text-muted-foreground">
                  {compressionLevel}
                </div>
              </div>
              <input
                type="range"
                min={1}
                max={100}
                value={compressionLevel}
                onChange={(event) =>
                  handleCompressionLevelChange(parseInt(event.target.value, 10))
                }
                className="w-full accent-primary"
                disabled={!compressionEnabled}
              />
              <p className="text-xs text-muted-foreground">
                Низкие уровни — быстрее, высокие — компактнее. Рекомендация: 40–70 для баланса.
              </p>
            </div>

            <div className="flex items-center justify-between gap-3 rounded-xl bg-background/30 px-2 py-2 hover:bg-accent/50 transition-colors">
              <button
                type="button"
                className="flex-1 cursor-pointer text-left disabled:cursor-not-allowed"
                disabled={!compressionEnabled}
                onClick={() => setSkipCompressionOnce((prev) => !prev)}
              >
                <span id="setting-skip-compression" className="text-sm">
                  Пропустить сжатие один раз
                </span>
              </button>
              <Switch
                checked={skipCompressionOnce}
                onCheckedChange={setSkipCompressionOnce}
                aria-labelledby="setting-skip-compression"
                disabled={!compressionEnabled}
              />
            </div>
            <p className="text-xs text-muted-foreground">
              Следующий бэкап будет создан без сжатия, а затем настройка вернётся автоматически.
            </p>
          </div>
        </section>

        <section id="settings-rawg" className="space-y-4">
          <div className="flex items-center gap-2">
            <Key className="h-5 w-5" />
            <h2 className="text-lg font-semibold">RAWG API</h2>
          </div>

          <div className="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
            <div>
              <label
                className="mb-2 block text-sm font-medium"
                htmlFor="rawg-api-key"
              >
                API ключ (необязательно)
              </label>
              <Input
                id="rawg-api-key"
                type="password"
                value={rawgApiKey}
                onChange={(event) => setRawgApiKey(event.target.value)}
                placeholder="Ваш RAWG API ключ"
              />
              <p className="mt-2 text-xs text-muted-foreground">
                Получите бесплатный ключ на{" "}
                <a
                  href="https://rawg.io/apidocs"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1 text-primary hover:underline"
                >
                  RAWG.io <ExternalLink className="h-3 w-3" />
                </a>{" "}
                для расширенных возможностей поиска
              </p>
            </div>
          </div>
        </section>

        <div className="fixed inset-x-0 bottom-0 z-10 border-t border-border/70 bg-background/85 p-4 backdrop-blur-md sm:static sm:border-none sm:bg-transparent sm:p-0">
          <div className="flex justify-end">
            <Button onClick={saveSettings} disabled={saving} className="gap-2 w-full sm:w-auto">
              {saving ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Check className="h-4 w-4" />
              )}
              Сохранить настройки
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
