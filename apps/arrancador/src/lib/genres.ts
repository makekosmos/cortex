const genreRuMap: Record<string, string> = {
  action: "Экшен",
  adventure: "Приключение",
  indie: "Инди",
  rpg: "RPG",
  shooter: "Шутер",
  strategy: "Стратегия",
  simulation: "Симулятор",
  puzzle: "Головоломка",
  platformer: "Платформер",
  racing: "Гонки",
  sports: "Спорт",
  fighting: "Файтинг",
  arcade: "Аркада",
  casual: "Казуальная",
  stealth: "Стелс",
  horror: "Хоррор",
  mmo: "MMO",
  multiplayer: "Мультиплеер",
  family: "Для семьи",
  educational: "Обучающая",
};

export function translateGenreToRu(value: string) {
  const normalized = value.trim().toLowerCase();
  return genreRuMap[normalized] ?? value.trim();
}

export function translateGenreListToRu(value: string | null | undefined, limit?: number) {
  if (!value) return [];

  const genres = value
    .split(",")
    .map((genre) => translateGenreToRu(genre))
    .filter(Boolean);

  return typeof limit === "number" ? genres.slice(0, limit) : genres;
}
