import { ExternalLink, Gamepad2, Loader2, Search, Star, Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useToast } from "@/components/ToastProvider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { gamesApi, metadataApi } from "@/lib/api";
import { translateGenreListToRu } from "@/lib/genres";
import { cn } from "@/lib/utils";
import { useGamesActions, useGamesState } from "@/store/GamesContext";
import type { RawgGame } from "@/types";

function sanitizeForExeName(value: string) {
  return value
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80);
}

function normalizeName(value: string) {
  return value.trim().toLowerCase();
}

export default function CataloguePage() {
  const { notify } = useToast();
  const { games } = useGamesState();
  const { addGame, refreshGames } = useGamesActions();

  const [query, setQuery] = useState("");
  const [items, setItems] = useState<RawgGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [searching, setSearching] = useState(false);
  const [addingId, setAddingId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestSeqRef = useRef(0);

  const libraryByRawgId = useMemo(() => {
    const map = new Map<number, string>();
    for (const game of games) {
      if (game.rawg_id) map.set(game.rawg_id, game.id);
    }
    return map;
  }, [games]);

  const libraryByName = useMemo(() => {
    const map = new Map<string, string>();
    for (const game of games) {
      map.set(normalizeName(game.name), game.id);
    }
    return map;
  }, [games]);

  const loadShowcase = useCallback(
    async (searchValue: string, mode: "initial" | "search" = "search") => {
      const requestId = ++requestSeqRef.current;

      if (mode === "initial") {
        setLoading(true);
      } else {
        setSearching(true);
        setLoading(false);
      }

      setError(null);

      try {
        const result = await metadataApi.search(searchValue.trim());
        if (requestSeqRef.current === requestId) {
          setItems(result);
        }
      } catch (e) {
        if (requestSeqRef.current === requestId) {
          console.error("Failed to load RAWG catalogue:", e);
          setError(
            mode === "initial"
              ? "Failed to load RAWG showcase"
              : "Failed to search RAWG",
          );
          if (mode === "search") {
            notify({ tone: "error", title: "RAWG search failed" });
          }
        }
      } finally {
        if (requestSeqRef.current === requestId) {
          if (mode === "initial") {
            setLoading(false);
          } else {
            setSearching(false);
          }
        }
      }
    },
    [notify],
  );

  useEffect(() => {
    void loadShowcase("", "initial");
  }, [loadShowcase]);

  const handleSearch = useCallback(async () => {
    await loadShowcase(query, "search");
  }, [loadShowcase, query]);

  const isInLibrary = useCallback(
    (item: RawgGame) => {
      if (libraryByRawgId.has(item.id)) return true;
      return libraryByName.has(normalizeName(item.name));
    },
    [libraryByName, libraryByRawgId],
  );

  const getLibraryGameLink = useCallback(
    (item: RawgGame) => {
      const byRawg = libraryByRawgId.get(item.id);
      if (byRawg) return `/game/${byRawg}`;
      const byName = libraryByName.get(normalizeName(item.name));
      if (byName) return `/game/${byName}`;
      return null;
    },
    [libraryByName, libraryByRawgId],
  );

  const handleAddToLibrary = useCallback(
    async (item: RawgGame) => {
      if (isInLibrary(item)) return;

      setAddingId(item.id);
      let createdGameId: string | null = null;

      try {
        const safeName = sanitizeForExeName(item.name) || `rawg-${item.id}`;
        const exeName = `${safeName}.exe`;
        const exePath = `C:\\Arrancador\\RAWG\\${item.id}\\${exeName}`;

        const game = await addGame({
          name: item.name,
          exe_name: exeName,
          exe_path: exePath,
        });
        createdGameId = game.id;

        await metadataApi.apply(game.id, item.id, true);
        await refreshGames();

        notify({
          tone: "success",
          title: "Game added to library",
          description: item.name,
        });
      } catch (e) {
        console.error("Failed to add game from RAWG:", e);

        if (createdGameId) {
          try {
            await gamesApi.delete(createdGameId);
            await refreshGames();
          } catch (cleanupError) {
            console.error("Failed to roll back placeholder game:", cleanupError);
          }
        }

        notify({
          tone: "error",
          title: "Failed to add game",
          description: item.name,
        });
      } finally {
        setAddingId(null);
      }
    },
    [addGame, isInLibrary, notify, refreshGames],
  );

  return (
    <div className="p-4 sm:p-6 max-w-7xl mx-auto space-y-6">
      <div className="space-y-2">
        <h1 className="text-2xl font-bold">Game Catalogue</h1>
        <p className="text-sm text-muted-foreground">
          Popular games from RAWG. Search and add them to your library.
        </p>
      </div>

      <div className="rounded-lg border bg-card/60 p-4 space-y-3">
        <div className="grid sm:grid-cols-[1fr_auto_auto] gap-3">
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search games by title"
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                void handleSearch();
              }
            }}
          />
          <Button
            variant="outline"
            onClick={() => {
              setQuery("");
              void loadShowcase("", "search");
            }}
            disabled={loading || searching}
          >
            Popular
          </Button>
          <Button onClick={() => void handleSearch()} disabled={searching}>
            {searching ? (
              <Loader2 className="w-4 h-4 animate-spin mr-2" />
            ) : (
              <Search className="w-4 h-4 mr-2" />
            )}
            Search
          </Button>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-destructive/40 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      ) : null}

      {loading || (searching && items.length === 0) ? (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="w-8 h-8 animate-spin" />
        </div>
      ) : items.length === 0 ? (
        <div className="rounded-lg border bg-card/40 p-8 text-center text-muted-foreground">
          No results. Try a different query.
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {items.map((item) => {
            const inLibrary = isInLibrary(item);
            const libraryLink = getLibraryGameLink(item);
            const genres = item.genres
              ?.map((g) => g.name)
              .slice(0, 2)
              .map((genre) => translateGenreListToRu(genre, 1)[0] ?? genre)
              .join(", ");
            const releaseYear = item.released?.slice(0, 4) || "----";
            const isAdding = addingId === item.id;

            return (
              <div
                key={item.id}
                className="rounded-xl border bg-card/60 overflow-hidden hover:border-primary/40 transition-colors"
              >
                <div className="relative aspect-[16/10] bg-muted">
                  {item.background_image ? (
                    <img
                      src={item.background_image}
                      alt={item.name}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <div className="w-full h-full flex items-center justify-center">
                      <Gamepad2 className="w-10 h-10 text-muted-foreground" />
                    </div>
                  )}

                  {item.metacritic ? (
                    <div
                      className={cn(
                        "absolute top-2 left-2 px-2 py-1 rounded text-xs font-bold",
                        item.metacritic >= 75
                          ? "bg-green-500 text-white"
                          : item.metacritic >= 50
                            ? "bg-yellow-500 text-black"
                            : "bg-red-500 text-white",
                      )}
                    >
                      {item.metacritic}
                    </div>
                  ) : null}
                </div>

                <div className="p-3 space-y-3">
                  <div>
                    <div className="font-semibold line-clamp-1">{item.name}</div>
                    <div className="text-xs text-muted-foreground">
                      {releaseYear}
                      {genres ? ` | ${genres}` : ""}
                    </div>
                    {typeof item.rating === "number" ? (
                      <div className="text-xs text-muted-foreground flex items-center gap-1 mt-1">
                        <Star className="w-3 h-3" />
                        {item.rating.toFixed(1)}
                      </div>
                    ) : null}
                  </div>

                  <div className="flex items-center gap-2">
                    {inLibrary && libraryLink ? (
                      <Button asChild className="flex-1 w-full" variant="secondary">
                        <Link to={libraryLink}>Open in library</Link>
                      </Button>
                    ) : (
                      <Button
                        className="flex-1"
                        onClick={() => void handleAddToLibrary(item)}
                        disabled={isAdding}
                      >
                        {isAdding ? (
                          <Loader2 className="w-4 h-4 animate-spin mr-2" />
                        ) : (
                          <Plus className="w-4 h-4 mr-2" />
                        )}
                        Add to library
                      </Button>
                    )}

                    <Button asChild variant="outline" size="icon" title="Open RAWG">
                      <a
                        href={`https://rawg.io/games/${item.slug}`}
                        target="_blank"
                        rel="noreferrer"
                      >
                        <ExternalLink className="w-4 h-4" />
                      </a>
                    </Button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
