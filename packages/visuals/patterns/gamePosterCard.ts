export const gamePosterCardClasses = {
  root: [
    "group relative isolate block aspect-video overflow-hidden rounded-xl",
    "shadow-[0_1px_0_color-mix(in_srgb,var(--destructive-foreground)_2%,transparent),0_16px_40px_color-mix(in_srgb,var(--background)_72%,transparent)]",
    "transition-colors duration-200",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]",
    "focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--background)]",
  ].join(" "),
  media: "isolate absolute inset-0 z-0 overflow-hidden",
  image: "absolute inset-0 z-0 h-full w-full object-cover",
  placeholder:
    "absolute inset-0 z-0 flex items-center justify-center bg-gradient-to-br from-(--muted) via-(--card) to-(--secondary)",
  scrim:
    "pointer-events-none absolute inset-x-0 bottom-0 z-10 h-[62%] bg-linear-to-t from-[color-mix(in_srgb,var(--background)_82%,transparent)] via-[color-mix(in_srgb,var(--background)_48%,transparent)] to-transparent",
  overlay:
    "pointer-events-none absolute inset-0 z-20 bg-(--background) opacity-0 transition-opacity duration-200 group-hover:opacity-40 group-focus-visible:opacity-40",
  content: "pointer-events-none absolute inset-x-0 bottom-0 z-30 flex flex-col gap-2 p-4 sm:p-6",
  eyebrow: "truncate text-xs font-normal leading-none text-white sm:text-sm",
  title:
    "line-clamp-2 overflow-hidden text-base font-semibold leading-tight text-[var(--destructive-foreground)] [text-shadow:0_1px_2px_color-mix(in_srgb,var(--background)_38%,transparent)] sm:text-lg",
} as const;
