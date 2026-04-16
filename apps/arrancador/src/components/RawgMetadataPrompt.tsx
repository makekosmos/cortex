import { ExternalLink, Gamepad2, Loader2, Search, X } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useToast } from "@/components/ToastProvider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { metadataApi } from "@/lib/api";
import type { Game, RawgGame } from "@/types";

type GameLite = Pick<Game, "id" | "name">;

interface RawgMetadataPromptProps {
  game: GameLite;
  remaining: number;
  onNext: () => void;
  onSkipAll: () => void;
  onAfterApply?: () => void | Promise<void>;
}

export function RawgMetadataPrompt({
  game,
  remaining,
  onNext,
  onSkipAll,
  onAfterApply,
}: RawgMetadataPromptProps) {
  const { notify } = useToast();

  const [query, setQuery] = useState(game.name);
  const [results, setResults] = useState<RawgGame[]>([]);
  const [searching, setSearching] = useState(false);
  const [applyingId, setApplyingId] = useState<number | null>(null);
  const [renameFromMetadata, setRenameFromMetadata] = useState(false);
  const [rawgKeyStatus, setRawgKeyStatus] = useState<
    "unknown" | "present" | "missing"
  >("unknown");

  const searchSeq = useRef(0);
  const applySeq = useRef(0);
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogId = useId();

  const hasApiKey = useMemo(() => rawgKeyStatus === "present", [rawgKeyStatus]);

  useEffect(() => {
    setQuery(game.name);
    setResults([]);
    setSearching(false);
    setApplyingId(null);

    let cancelled = false;
    setRawgKeyStatus("unknown");
    metadataApi
      .getApiKey()
      .then((key) => {
        if (cancelled) return;
        setRawgKeyStatus(key?.trim() ? "present" : "missing");
      })
      .catch(() => {
        if (cancelled) return;
        setRawgKeyStatus("missing");
      });

    return () => {
      cancelled = true;
    };
  }, [game.id, game.name]);

  useEffect(() => {
    if (!hasApiKey) return;
    if (!query.trim()) return;

    const timeout = window.setTimeout(() => {
      void searchMetadata(query);
    }, 80);

    return () => {
      window.clearTimeout(timeout);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id, hasApiKey]);

  useEffect(() => {
    const handle = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(handle);
  }, []);

  useEffect(() => {
    const getFocusableElements = () =>
      Array.from(
        panelRef.current?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onNext();
        return;
      }

      if (event.key !== "Tab") return;

      const focusables = getFocusableElements();
      if (focusables.length === 0) return;

      const currentIndex = focusables.indexOf(document.activeElement as HTMLElement);
      const nextIndex = event.shiftKey
        ? currentIndex <= 0
          ? focusables.length - 1
          : currentIndex - 1
        : currentIndex === focusables.length - 1
          ? 0
          : currentIndex + 1;

      event.preventDefault();
      focusables[nextIndex]?.focus();
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onNext]);

  const searchMetadata = async (forcedQuery?: string) => {
    const q = (forcedQuery ?? query).trim();
    if (!q) return;

    const seq = ++searchSeq.current;
    setSearching(true);
    try {
      const found = await metadataApi.search(q);
      if (searchSeq.current !== seq) return;
      setResults(found);
      if (found.length === 0) {
        notify({
          tone: "info",
          title: "Ничего не найдено",
          description: "Попробуйте уточнить запрос.",
          durationMs: 2600,
        });
      }
    } catch (error) {
      if (searchSeq.current !== seq) return;
      console.error("RAWG search failed:", error);
      notify({
        tone: "error",
        title: "Не удалось найти метаданные",
        description: "Проверьте RAWG API ключ в настройках.",
      });
    } finally {
      if (searchSeq.current === seq) {
        setSearching(false);
      }
    }
  };

  const applyMetadata = async (rawgGame: RawgGame) => {
    const seq = ++applySeq.current;
    setApplyingId(rawgGame.id);
    try {
      await metadataApi.apply(game.id, rawgGame.id, renameFromMetadata);
      if (applySeq.current !== seq) return;
      await onAfterApply?.();
      notify({
        tone: "success",
        title: "Метаданные применены",
        description: rawgGame.name,
        durationMs: 2400,
      });
      onNext();
    } catch (error) {
      if (applySeq.current !== seq) return;
      console.error("Failed to apply RAWG metadata:", error);
      notify({
        tone: "error",
        title: "Не удалось применить метаданные",
      });
    } finally {
      if (applySeq.current === seq) {
        setApplyingId(null);
      }
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={`${dialogId}-title`}
      aria-describedby={`${dialogId}-description`}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm animate-in fade-in duration-200"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onNext();
        }
      }}
    >
      <div
        ref={panelRef}
        className="relative flex w-full max-w-xl max-h-[85vh] flex-col overflow-hidden rounded-2xl border border-border/70 bg-card shadow-[0_30px_80px_rgba(0,0,0,0.45)]"
      >
        <div className="pointer-events-none absolute inset-0">
          <div
            className="absolute -top-24 -right-24 h-56 w-56 rounded-full blur-3xl opacity-60"
            style={{
              background:
                "radial-gradient(circle, rgba(56,189,248,0.22), rgba(56,189,248,0))",
            }}
          />
          <div
            className="absolute -bottom-24 -left-24 h-56 w-56 rounded-full blur-3xl opacity-50"
            style={{
              background:
                "radial-gradient(circle, rgba(244,63,94,0.18), rgba(244,63,94,0))",
            }}
          />
        </div>

        <div className="relative border-b border-border/70 p-4">
          <div className="flex items-start gap-3">
            <div className="min-w-0 flex-1">
              <div id={`${dialogId}-title`} className="text-sm font-semibold">
                Добавить метаданные из RAWG
              </div>
              <div
                id={`${dialogId}-description`}
                className="mt-0.5 truncate text-xs text-muted-foreground"
              >
                {game.name}
                {remaining > 1 ? ` · Осталось: ${remaining}` : ""}
              </div>
            </div>
            <button
              type="button"
              onClick={onSkipAll}
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Закрыть"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        </div>

        <div className="relative flex flex-1 flex-col overflow-hidden p-4">
          {rawgKeyStatus === "missing" && (
            <div className="mb-4 rounded-xl border border-border/70 bg-secondary/20 p-3">
              <div className="text-sm font-semibold">Нужен RAWG API ключ</div>
              <div className="mt-1 text-xs leading-relaxed text-muted-foreground">
                Добавьте ключ в настройках, чтобы искать метаданные.
              </div>
              <div className="mt-3 flex items-center justify-end gap-2">
                <Button variant="ghost" onClick={onNext}>
                  Не сейчас
                </Button>
                <Button asChild>
                  <Link to="/settings">Открыть настройки</Link>
                </Button>
              </div>
            </div>
          )}

          <div className="mb-4 flex gap-2">
            <Input
              ref={inputRef}
              placeholder="Название игры..."
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && void searchMetadata()}
              disabled={!hasApiKey}
              autoFocus
              aria-label="Название игры"
              className="h-10"
            />
            <Button
              onClick={() => void searchMetadata()}
              disabled={!hasApiKey || searching}
              title={hasApiKey ? "Найти" : "Нужен RAWG API ключ"}
            >
              {searching ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
            </Button>
          </div>

          <div className="mb-3 flex items-center justify-between gap-3 text-sm text-muted-foreground">
            <span id="rawg-rename-toggle">Использовать название из RAWG</span>
            <Switch
              checked={renameFromMetadata}
              onCheckedChange={setRenameFromMetadata}
              aria-labelledby="rawg-rename-toggle"
            />
          </div>

          <ScrollArea className="flex-1">
            {results.length > 0 ? (
              <div className="space-y-2">
                {results.map((result) => (
                  <button
                    type="button"
                    key={result.id}
                    className="flex w-full items-center gap-3 rounded-xl border border-transparent p-3 text-left transition-colors hover:border-border hover:bg-secondary/70 disabled:cursor-not-allowed disabled:opacity-70"
                    onClick={() => void applyMetadata(result)}
                    disabled={applyingId !== null}
                  >
                    {result.background_image ? (
                      <img
                        src={result.background_image}
                        alt={result.name}
                        className="h-14 w-14 rounded-lg border border-border/60 object-cover"
                        loading="lazy"
                        decoding="async"
                      />
                    ) : (
                      <div className="flex h-14 w-14 items-center justify-center rounded-lg border border-border/60 bg-muted">
                        <Gamepad2 className="h-6 w-6 text-muted-foreground" />
                      </div>
                    )}

                    <div className="min-w-0 flex-1">
                      <div className="truncate font-medium">{result.name}</div>
                      <div className="mt-0.5 text-xs text-muted-foreground">
                        {result.released?.slice(0, 4) ?? "—"}
                        {result.metacritic != null ? ` · MC ${result.metacritic}` : ""}
                      </div>
                    </div>

                    {applyingId === result.id ? (
                      <Loader2 className="h-4 w-4 animate-spin text-primary" />
                    ) : (
                      <ExternalLink className="h-4 w-4 text-muted-foreground" />
                    )}
                  </button>
                ))}
              </div>
            ) : (
              <div className="py-10 text-center text-sm text-muted-foreground">
                {hasApiKey
                  ? "Введите название и нажмите поиск"
                  : "Добавьте RAWG API ключ в настройках"}
              </div>
            )}
          </ScrollArea>
        </div>

        <div className="relative flex items-center justify-between gap-2 border-t border-border/70 p-4">
          <Button variant="ghost" onClick={onSkipAll}>
            Пропустить все
          </Button>
          <div className="flex items-center gap-2">
            <Button variant="ghost" onClick={onNext}>
              Пропустить
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
