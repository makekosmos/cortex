import { Clock, Search, X } from "lucide-react";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import { useLanguage } from "@/components/language-provider";
import { cn } from "@/lib/utils";
import { useGamesState } from "@/store/GamesContext";
import type { Game } from "@/types";

const MAX_RESULTS = 12;

function formatPlaytime(seconds: number, language: "ru" | "en") {
  if (!seconds) return language === "ru" ? "0 \u0447" : "0 h";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);

  if (hours > 0) {
    return language === "ru"
      ? `${hours} \u0447 ${minutes} \u043c\u0438\u043d`
      : `${hours} h ${minutes} m`;
  }

  return language === "ru" ? `${minutes} \u043c\u0438\u043d` : `${minutes} m`;
}

function makeMatchScore(game: Game, query: string) {
  const normalized = query.toLowerCase();
  const name = game.name.toLowerCase();
  const exeName = game.exe_name.toLowerCase();

  if (name === normalized || exeName === normalized) return 0;
  if (name.startsWith(normalized) || exeName.startsWith(normalized)) return 1;
  if (name.includes(normalized) || exeName.includes(normalized)) return 2;
  return Number.POSITIVE_INFINITY;
}

type SpotlightProps = {
  triggerClassName?: string;
  showTrigger?: boolean;
  triggerVariant?: "default" | "sidebar";
  enableShortcut?: boolean;
};

export default function Spotlight({
  triggerClassName,
  showTrigger = true,
  triggerVariant = "default",
  enableShortcut = true,
}: SpotlightProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const { games, loading } = useGamesState();
  const { language } = useLanguage();

  const [open, setOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const previousRouteRef = useRef(
    `${location.pathname}${location.search}${location.hash}`,
  );
  const instanceId = useId();

  const labels = useMemo(
    () =>
      language === "ru"
        ? {
            button: "\u041f\u043e\u0438\u0441\u043a",
            placeholder: "\u041f\u043e\u0438\u0441\u043a \u0438\u0433\u0440...",
            close: "\u0417\u0430\u043a\u0440\u044b\u0442\u044c",
            title: "\u0411\u044b\u0441\u0442\u0440\u044b\u0439 \u043f\u043e\u0438\u0441\u043a",
            loading: "\u0417\u0430\u0433\u0440\u0443\u0437\u043a\u0430 \u0431\u0438\u0431\u043b\u0438\u043e\u0442\u0435\u043a\u0438...",
            noQuery:
              "\u0412 \u0431\u0438\u0431\u043b\u0438\u043e\u0442\u0435\u043a\u0435 \u043f\u043e\u043a\u0430 \u043d\u0435\u0442 \u0438\u0433\u0440",
            noMatch:
              "\u041d\u0435\u0442 \u043f\u043e\u0434\u0445\u043e\u0434\u044f\u0449\u0438\u0445 \u0438\u0433\u0440",
          }
        : {
            button: "Search",
            placeholder: "Search games...",
            close: "Close",
            title: "Quick search",
            loading: "Loading library...",
            noQuery: "No games in library yet",
            noMatch: "No matching games",
          },
    [language],
  );

  const close = useCallback(() => {
    setOpen(false);
    setSearchQuery("");
    setActiveIndex(0);
  }, []);

  const openPanel = useCallback(() => {
    setSearchQuery("");
    setActiveIndex(0);
    setOpen(true);
  }, []);

  const filteredGames = useMemo(() => {
    if (loading) return [];
    const query = searchQuery.trim().toLowerCase();

    if (!query) {
      return [...games]
        .sort((a, b) => {
          if (a.is_favorite !== b.is_favorite) {
            return a.is_favorite ? -1 : b.is_favorite ? 1 : 0;
          }

          return a.name.localeCompare(b.name);
        })
        .slice(0, MAX_RESULTS);
    }

    const scored = games
      .map((game) => ({
        game,
        score: makeMatchScore(game, query),
      }))
      .filter((entry) => Number.isFinite(entry.score));

    scored.sort((a, b) => {
      if (a.score !== b.score) return a.score - b.score;
      if (a.game.is_favorite !== b.game.is_favorite) {
        return a.game.is_favorite ? -1 : b.game.is_favorite ? 1 : 0;
      }

      return a.game.name.localeCompare(b.game.name);
    });

    return scored.slice(0, MAX_RESULTS).map((entry) => entry.game);
  }, [games, loading, searchQuery]);

  useEffect(() => {
    if (filteredGames.length <= activeIndex) {
      setActiveIndex(0);
    }
  }, [activeIndex, filteredGames.length]);

  useEffect(() => {
    if (!open || filteredGames.length === 0) {
      return;
    }

    optionRefs.current[activeIndex]?.scrollIntoView({
      block: "nearest",
    });
  }, [activeIndex, filteredGames.length, open]);

  useEffect(() => {
    if (!open) return;
    const handle = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(handle);
  }, [open]);

  useEffect(() => {
    const nextRoute = `${location.pathname}${location.search}${location.hash}`;
    const previousRoute = previousRouteRef.current;
    previousRouteRef.current = nextRoute;

    if (!open || previousRoute === nextRoute) {
      return;
    }

    close();
  }, [close, location.hash, location.pathname, location.search, open]);

  useEffect(() => {
    const getFocusableElements = () =>
      Array.from(
        panelRef.current?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );

    const onKeyDown = (event: KeyboardEvent) => {
      if (
        enableShortcut &&
        (event.ctrlKey || event.metaKey) &&
        event.code === "KeyK"
      ) {
        event.preventDefault();
        if (open) {
          close();
        } else {
          openPanel();
        }
        return;
      }

      if (!open) return;

      if (event.key === "Escape") {
        event.preventDefault();
        close();
        return;
      }

      if (event.key === "Tab") {
        const focusables = getFocusableElements();
        if (focusables.length === 0) return;

        const currentIndex = focusables.indexOf(
          document.activeElement as HTMLElement,
        );
        const nextIndex = event.shiftKey
          ? currentIndex <= 0
            ? focusables.length - 1
            : currentIndex - 1
          : currentIndex === focusables.length - 1
            ? 0
            : currentIndex + 1;

        event.preventDefault();
        focusables[nextIndex]?.focus();
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setActiveIndex((value) =>
          filteredGames.length === 0
            ? 0
            : Math.min(filteredGames.length - 1, value + 1),
        );
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setActiveIndex((value) => Math.max(0, value - 1));
      } else if (event.key === "Enter") {
        if (!filteredGames[activeIndex]) return;
        event.preventDefault();
        const target = filteredGames[activeIndex];
        close();
        navigate(`/game/${target.id}`);
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [activeIndex, close, enableShortcut, filteredGames, navigate, open, openPanel]);

  const handleSelect = useCallback(
    (game: Game) => {
      close();
      navigate(`/game/${game.id}`);
    },
    [close, navigate],
  );

  if (!open) {
    if (!showTrigger) return null;

    return (
      <button
        type="button"
        onClick={openPanel}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={`spotlight-${instanceId}`}
        className={cn(
          triggerVariant === "sidebar"
            ? "kepler-sidebar-btn"
            : "inline-flex h-9 items-center gap-2 rounded-md border border-border/70 bg-card/70 px-3 text-xs font-[510] text-muted-foreground shadow-[0_1px_0_rgba(255,255,255,0.02)] backdrop-blur-md transition-colors hover:border-border hover:bg-card hover:text-foreground",
          triggerClassName,
        )}
      >
        <Search className="h-4 w-4" />
        <span
          className={cn(
            triggerVariant === "sidebar" ? "inline truncate" : "hidden sm:inline",
          )}
        >
          {labels.button}
        </span>
      </button>
    );
  }

  return (
    <div
      ref={overlayRef}
      id={`spotlight-${instanceId}`}
      role="dialog"
      aria-modal="true"
      aria-labelledby={`spotlight-${instanceId}-title`}
      aria-describedby={`spotlight-${instanceId}-desc`}
      className="fixed inset-0 z-[90] flex items-start justify-center bg-black/84 px-4 pt-16 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === overlayRef.current) {
          close();
        }
      }}
    >
      <div
        ref={panelRef}
        className="w-full max-w-2xl overflow-hidden rounded-2xl border border-border/70 bg-card/96 shadow-[0_30px_80px_rgba(0,0,0,0.45)] backdrop-blur-xl"
      >
        <div className="flex items-center gap-2 border-b border-border/70 px-3 py-2.5">
          <Search className="h-4 w-4 text-muted-foreground" />
          <input
            ref={inputRef}
            value={searchQuery}
            onChange={(event) => {
              setSearchQuery(event.target.value);
              setActiveIndex(0);
            }}
            placeholder={labels.placeholder}
            aria-label={labels.placeholder}
            aria-describedby={`spotlight-${instanceId}-desc`}
            className="w-full bg-transparent py-2 text-sm text-foreground outline-none placeholder:text-muted-foreground"
          />
          <button
            type="button"
            onClick={close}
            aria-label={labels.close}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-accent/80 hover:text-foreground"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <p id={`spotlight-${instanceId}-desc`} className="sr-only">
          {labels.title}
        </p>

        <div className="max-h-[60vh] overflow-y-auto p-2">
          {loading ? (
            <div className="px-3 py-8 text-center text-sm text-muted-foreground">
              {labels.loading}
            </div>
          ) : filteredGames.length === 0 ? (
            <div className="px-3 py-8 text-center text-sm text-muted-foreground">
              {searchQuery.trim() ? labels.noMatch : labels.noQuery}
            </div>
          ) : (
            <ul className="space-y-1" role="listbox" aria-label={labels.title}>
              {filteredGames.map((game, index) => (
                <li key={game.id}>
                  <button
                    ref={(node) => {
                      optionRefs.current[index] = node;
                    }}
                    type="button"
                    role="option"
                    data-spotlight-option-index={index}
                    aria-selected={index === activeIndex}
                    onClick={() => handleSelect(game)}
                    onMouseEnter={() => setActiveIndex(index)}
                    className={cn(
                      "group flex w-full items-center gap-3 rounded-xl border px-2 py-2 text-left transition-colors",
                      index === activeIndex
                        ? "border-border/80 bg-accent/80 text-accent-foreground shadow-[0_1px_0_rgba(255,255,255,0.02)]"
                        : "border-transparent text-foreground/90 hover:border-border/70 hover:bg-accent/60",
                    )}
                  >
                    <div className="flex h-10 w-10 flex-shrink-0 overflow-hidden rounded-lg bg-muted">
                      {game.background_image ? (
                        <img
                          src={game.background_image}
                          alt={game.name}
                          className="h-full w-full object-cover"
                          loading="lazy"
                          decoding="async"
                        />
                      ) : null}
                    </div>

                    <div className="min-w-0 flex-1">
                      <div className="truncate font-[510]">{game.name}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {game.exe_name}
                      </div>
                    </div>

                    {game.total_playtime > 0 ? (
                      <div className="hidden items-center gap-1 text-xs text-muted-foreground sm:flex">
                        <Clock className="h-3 w-3" />
                        {formatPlaytime(game.total_playtime, language)}
                      </div>
                    ) : null}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
