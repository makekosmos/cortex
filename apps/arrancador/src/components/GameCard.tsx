import { Gamepad2, Play } from "lucide-react";
import { Link } from "react-router-dom";
import type { Game } from "@/types";

interface GameCardProps {
  game: Game;
}

export function GameCard({ game }: GameCardProps) {
  const cover = game.cover_image || game.background_image;

  return (
    <Link
      to={`/game/${game.id}`}
      aria-label={game.name}
      title={game.name}
      className="group relative block aspect-[3/4] overflow-hidden rounded-xl border border-border/70 bg-card/80 shadow-[0_1px_0_rgba(255,255,255,0.02),0_18px_40px_rgba(0,0,0,0.26)] transition-all duration-200 hover:-translate-y-0.5 hover:border-border hover:shadow-[0_1px_0_rgba(255,255,255,0.02),0_24px_55px_rgba(0,0,0,0.34)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    >
      {cover ? (
        <img
          src={cover}
          alt={game.name}
          className="absolute inset-0 h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
          loading="lazy"
          decoding="async"
        />
      ) : (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-muted via-card to-secondary">
          <Gamepad2 className="h-12 w-12 text-muted-foreground" />
        </div>
      )}

      <div
        className="absolute inset-0 bg-gradient-to-t from-black/88 via-black/30 to-transparent"
        aria-hidden="true"
      />

      <div className="absolute bottom-0 left-0 right-0 p-3">
        <h3 className="truncate-2 text-sm font-[510] leading-tight text-white">
          {game.name}
        </h3>
        <div className="sr-only">{game.name}</div>
      </div>

      <div className="absolute inset-0 flex items-center justify-center bg-black/18 opacity-0 transition-opacity duration-200 group-hover:opacity-100 group-focus-visible:opacity-100">
        <div className="flex h-14 w-14 items-center justify-center rounded-full border border-white/18 bg-white/8 backdrop-blur-sm">
          <Play className="ml-1 h-6 w-6 fill-white text-white" />
        </div>
      </div>
    </Link>
  );
}
