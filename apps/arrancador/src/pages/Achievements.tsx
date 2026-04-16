import {
  Award,
  CheckCircle2,
  Gift,
  Loader2,
  RefreshCw,
  Sparkles,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { achievementsApi } from "@/lib/api";
import { useToast } from "@/components/ToastProvider";
import { cn } from "@/lib/utils";
import type { Achievement } from "@/types";

type AchFilter = "all" | "unlocked" | "locked";

const eventTriggers = [
  "game_launch",
  "download_complete",
  "backup_restore",
  "scan_complete",
];

export default function AchievementsPage() {
  const { notify } = useToast();

  const [loading, setLoading] = useState(false);
  const [savingDefaults, setSavingDefaults] = useState(false);
  const [recording, setRecording] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [achievements, setAchievements] = useState<Achievement[]>([]);
  const [filter, setFilter] = useState<AchFilter>("all");
  const [eventName, setEventName] = useState(eventTriggers[0]);
  const [eventContext, setEventContext] = useState("");

  const unlockedCount = useMemo(
    () => achievements.filter((a) => a.unlocked).length,
    [achievements],
  );
  const totalCount = achievements.length;
  const lockedCount = totalCount - unlockedCount;
  const eventContextPreview =
    eventContext.trim().length > 0 ? eventContext.trim() : "No context";

  const unlockedRecent = useMemo(
    () =>
      [...achievements]
        .filter((item) => item.unlocked && item.unlocked_at)
        .sort((a, b) => b.unlocked_at!.localeCompare(a.unlocked_at!))
        .slice(0, 3),
    [achievements],
  );

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const base =
        filter === "unlocked"
          ? await achievementsApi.getAll(true)
          : await achievementsApi.getAll(false);

      setAchievements(
        filter === "locked"
          ? base.filter((item: Achievement) => !item.unlocked)
          : base,
      );
    } catch (err) {
      console.error("Failed to load achievements:", err);
      setError("Failed to load achievements");
      notify({
        title: "Failed to load achievements",
        tone: "error",
      });
    } finally {
      setLoading(false);
    }
  }, [filter, notify]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleSeedDefaults = useCallback(async () => {
    setSavingDefaults(true);
    try {
      await achievementsApi.seedDefaults();
      await load();
      notify({
        title: "Default achievements restored",
        tone: "success",
      });
    } catch (err) {
      console.error("Failed to seed defaults:", err);
      notify({
        title: "Failed to seed achievements",
        tone: "error",
      });
    } finally {
      setSavingDefaults(false);
    }
  }, [load, notify]);

  const handleRecordEvent = useCallback(async () => {
    setRecording(true);
    try {
      await achievementsApi.recordEvent(
        eventName,
        eventContext.trim() || undefined,
      );
      await load();
      notify({
        title: "Achievement event recorded",
        tone: "success",
      });
    } catch (err) {
      console.error("Failed to record achievement event:", err);
      notify({
        title: "Failed to record achievement event",
        tone: "error",
      });
    } finally {
      setRecording(false);
    }
  }, [eventContext, eventName, notify]);

  const handleExport = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(achievements, null, 2));
      notify({
        title: "Achievement export copied",
        tone: "success",
      });
    } catch {
      notify({
        title: "Failed to export achievements",
        tone: "error",
      });
    }
  }, [achievements, notify]);

  const handleImport = useCallback(async () => {
    const raw = window.prompt("Paste achievement JSON to import");
    if (!raw) return;

    setImporting(true);
    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        throw new Error("Invalid payload");
      }

      const normalized = parsed.filter((item) => {
        return (
          item &&
          typeof item.id === "string" &&
          typeof item.title === "string" &&
          typeof item.description === "string" &&
          typeof item.event_trigger === "string" &&
          typeof item.progress === "number" &&
          typeof item.target === "number" &&
          typeof item.unlocked === "boolean" &&
          (item.unlocked_at === null || typeof item.unlocked_at === "string") &&
          typeof item.created_at === "string"
        );
      }) as Achievement[];

      setAchievements(normalized);
      notify({
        title: "Imported achievements from clipboard JSON",
        tone: "success",
      });
    } catch {
      notify({
        title: "Invalid JSON payload",
        tone: "error",
      });
    } finally {
      setImporting(false);
    }
  }, [notify]);

  const handleRefresh = useCallback(() => {
    void load();
  }, [load]);

  return (
    <div className="p-4 sm:p-6 max-w-6xl mx-auto space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold">Achievements</h1>
          <p className="text-sm text-muted-foreground">
            Track local achievements and achievement-trigger events.
          </p>
        </div>

        <div className="flex gap-2 flex-wrap">
          <Button variant="outline" size="sm" onClick={handleRefresh} data-testid="achievements-refresh">
            <RefreshCw className="w-4 h-4 mr-2" />
            Refresh
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={handleSeedDefaults}
            disabled={savingDefaults}
            data-testid="achievements-seed"
          >
            {savingDefaults ? (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            ) : (
              <Sparkles className="w-4 h-4 mr-2" />
            )}
            Seed defaults
          </Button>
        </div>
      </div>

      <div className="rounded-xl border bg-card/60 p-4 space-y-3">
        <div className="text-sm text-muted-foreground" data-testid="achievements-kpi">
          Unlocked: {unlockedCount} / {totalCount} • Locked: {lockedCount}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant={filter === "all" ? "secondary" : "outline"}
            size="sm"
            onClick={() => setFilter("all")}
            data-testid="achievements-filter-all"
          >
            All
          </Button>
          <Button
            variant={filter === "unlocked" ? "secondary" : "outline"}
            size="sm"
            onClick={() => setFilter("unlocked")}
            data-testid="achievements-filter-unlocked"
          >
            Unlocked
          </Button>
          <Button
            variant={filter === "locked" ? "secondary" : "outline"}
            size="sm"
            onClick={() => setFilter("locked")}
            data-testid="achievements-filter-locked"
          >
            Locked
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleExport}
            data-testid="achievements-export"
          >
            Export
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={handleImport}
            disabled={importing}
            data-testid="achievements-import"
          >
            {importing ? "Importing..." : "Import from clipboard"}
          </Button>
          <span className="text-xs text-muted-foreground ml-2">
            Manual event recorder:
          </span>
          <select
            value={eventName}
            onChange={(event) => setEventName(event.target.value)}
            className="bg-background border rounded-md px-2 py-2 text-sm"
            data-testid="achievements-event-select"
          >
            {eventTriggers.map((trigger) => (
              <option key={trigger} value={trigger}>
                {trigger}
              </option>
            ))}
          </select>
          <Input
            value={eventContext}
            onChange={(event) => setEventContext(event.target.value)}
            placeholder="Context (optional)"
            className="max-w-[220px]"
            data-testid="achievements-context-input"
          />
          <Button
            onClick={handleRecordEvent}
            disabled={recording}
            size="sm"
            data-testid="achievements-record"
          >
            {recording ? (
              <Loader2 className="w-4 h-4 mr-2 animate-spin" />
            ) : (
              <Gift className="w-4 h-4 mr-2" />
            )}
            Record event
          </Button>
          <div
            className="text-xs text-muted-foreground w-full sm:w-auto"
            data-testid="achievements-context-preview"
          >
            Context preview: {eventContextPreview}
          </div>
        </div>
      </div>

      {error ? (
        <div
          className="rounded-lg border border-destructive/40 bg-destructive/5 p-4 text-sm text-destructive flex items-center justify-between gap-2"
          data-testid="achievements-error"
        >
          <span>{error}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={handleRefresh}
            data-testid="achievements-retry"
          >
            Try again
          </Button>
        </div>
      ) : null}

      {loading ? (
        <div
          className="text-sm text-muted-foreground flex items-center gap-2"
          data-testid="achievements-loading"
        >
          <Loader2 className="w-4 h-4 animate-spin" />
          Loading achievements...
        </div>
      ) : achievements.length === 0 ? (
        <div className="rounded-lg border border-dashed p-8 text-sm text-muted-foreground text-center">
          <span data-testid="achievements-empty">No achievements match current filter.</span>
        </div>
      ) : (
        <div className="space-y-4" data-testid="achievements-panel">
          <div>
            <div className="font-medium text-sm text-muted-foreground mb-2">
              Recent unlocks (timeline)
            </div>
            {unlockedRecent.length === 0 ? (
              <div className="text-xs text-muted-foreground">
                No unlocks yet.
              </div>
            ) : (
              <ul className="space-y-1 text-sm">
                {unlockedRecent.map((item) => (
                  <li
                    key={item.id}
                    className="flex items-center justify-between"
                  >
                    <span>{item.title}</span>
                    <span className="text-xs text-muted-foreground">
                      {new Date(item.unlocked_at ?? item.created_at).toLocaleString()}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
            {achievements.map((item) => {
              const percent =
                item.target > 0
                  ? Math.round((item.progress / item.target) * 100)
                  : 0;

              return (
                <div
                  key={item.id}
                  className="rounded-xl border p-4 bg-card/50 border-border/60 space-y-3"
                  data-testid={`achievement-card-${item.id}`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div>
                      <div className="font-medium">{item.title}</div>
                      <div className="text-xs text-muted-foreground mt-1">
                        Trigger: {item.event_trigger}
                      </div>
                    </div>

                    <span
                      className={cn(
                        "text-xs px-2 py-1 rounded-full font-medium",
                        item.unlocked
                          ? "bg-emerald-500/20 text-emerald-300"
                          : "bg-muted text-muted-foreground",
                      )}
                    >
                      {item.unlocked ? (
                        <>
                          <CheckCircle2 className="w-3 h-3 inline-block mr-1" />
                          unlocked
                        </>
                      ) : (
                        <>
                          <Award className="w-3 h-3 inline-block mr-1" />
                          locked
                        </>
                      )}
                    </span>
                  </div>

                  <p className="text-sm text-muted-foreground">
                    {item.description}
                  </p>

                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                      <span>
                        {item.progress}/{item.target} progress
                      </span>
                      <span>{percent}%</span>
                    </div>
                    <Progress value={percent} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
