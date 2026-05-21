export const gamePosterCardClasses = {
  root: [
    "kosmos-game-poster-card group relative isolate block overflow-hidden rounded-xl ",
    "shadow-[0_1px_0_rgba(255,255,255,0.02),0_18px_40px_rgba(0,0,0,0.26)]",
    "transition-colors duration-200",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/60",
    "focus-visible:ring-offset-2 focus-visible:ring-offset-background",
  ].join(" "),
  media: "kosmos-game-poster-card__media-stack absolute inset-0 z-0 overflow-hidden",
  image: "kosmos-game-poster-card__media absolute inset-0 z-0 h-full w-full object-cover",
  placeholder:
    "kosmos-game-poster-card__media absolute inset-0 z-0 flex items-center justify-center bg-gradient-to-br from-muted via-card to-secondary",
  scrim: "kosmos-game-poster-card__scrim pointer-events-none absolute inset-x-0 bottom-0 z-10",
  overlay: "kosmos-game-poster-card__overlay pointer-events-none absolute inset-0 z-20",
  content:
    "kosmos-game-poster-card__content pointer-events-none absolute inset-x-0 bottom-0 z-30 flex flex-col gap-1 p-4 sm:p-5",
  eyebrow: "kosmos-game-poster-card__eyebrow truncate text-xs font-normal leading-none sm:text-sm",
  title: "kosmos-game-poster-card__title text-base font-semibold leading-tight sm:text-lg",
} as const;
