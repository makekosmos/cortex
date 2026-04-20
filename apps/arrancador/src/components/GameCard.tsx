import { Gamepad2 } from "lucide-react";
import { Link } from "react-router-dom";
import { GamePosterCard } from "../../../../packages/kepler-visuals/react";

import { translateGenreListToRu } from "@/lib/genres";
import type { Game } from "@/types";

interface GameCardProps {
  game: Game;
}

export function GameCard({ game }: GameCardProps) {
  const cover = game.background_image || game.cover_image;
  const primaryGenre = translateGenreListToRu(game.genres, 1)[0] ?? null;

  return (
    <GamePosterCard
      to={`/game/${game.id}`}
      title={game.name}
      eyebrow={primaryGenre}
      coverSrc={cover}
      LinkComponent={Link}
      placeholder={<Gamepad2 className="h-12 w-12 text-muted-foreground" />}
    />
  );
}
