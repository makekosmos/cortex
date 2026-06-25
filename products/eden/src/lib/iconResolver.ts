// Резолвер object-type иконок: anytype-name → data:URI с Lucide SVG.
//
// Sidebar / type pickers / object cards принимают `iconSrc: string` и
// рендерят через `mask-image` (см. `kosmos-sidebar-project-icon`). Это
// значит SVG нужен monochrome (mask берёт alpha) и работает с любым
// stroke цветом — currentColor в `<svg stroke="currentColor">` подменяется
// внешним `background-color: var(...)`. Тут возвращаем именно такой
// data:URI.
//
// Имена иконок (`noteType.icon`) исторически совпадают с anytype icon ids:
// "document", "document-text", "game-controller", "image", "barbell",
// "fitness", "page", "book", "calendar", "planet", "library", "folder",
// "sparkles", "user". Mapping ниже покрывает встречающиеся значения; всё
// что не нашлось падает в `document` (FileText).
//
// SVG path data заимствован из `@lucide/vue` (v0.548) — встраиваем
// inline чтобы избежать tree-shake'а Lucide через Vue-component сборку
// и контроля над финальным attributes (stroke=currentColor для mask).

type IconNode = readonly (readonly [string, Record<string, string>])[];

const FILE_TEXT: IconNode = [
  ["path", { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }],
  ["path", { d: "M14 2v4a2 2 0 0 0 2 2h4" }],
  ["path", { d: "M10 9H8" }],
  ["path", { d: "M16 13H8" }],
  ["path", { d: "M16 17H8" }],
];

const FILE: IconNode = [
  ["path", { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }],
  ["path", { d: "M14 2v4a2 2 0 0 0 2 2h4" }],
];

const GAMEPAD_CONTROLLER: IconNode = [
  ["line", { x1: "6", x2: "10", y1: "11", y2: "11" }],
  ["line", { x1: "8", x2: "8", y1: "9", y2: "13" }],
  ["line", { x1: "15", x2: "15.01", y1: "12", y2: "12" }],
  ["line", { x1: "18", x2: "18.01", y1: "10", y2: "10" }],
  [
    "path",
    {
      d: "M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z",
    },
  ],
];

const IMAGE: IconNode = [
  ["rect", { x: "3", y: "3", width: "18", height: "18", rx: "2" }],
  ["circle", { cx: "8", cy: "8", r: "1.5" }],
  ["path", { d: "M21 15l-5-5L5 21" }],
  ["path", { d: "M3 17l5-5 3 3" }],
];

const DUMBBELL: IconNode = [
  [
    "path",
    {
      d: "M17.596 12.768a2 2 0 1 0 2.829-2.829l-1.768-1.767a2 2 0 0 0 2.828-2.829l-2.828-2.828a2 2 0 0 0-2.829 2.828l-1.767-1.768a2 2 0 1 0-2.829 2.829z",
    },
  ],
  ["path", { d: "m2.5 21.5 1.4-1.4" }],
  ["path", { d: "m20.1 3.9 1.4-1.4" }],
  [
    "path",
    {
      d: "M5.343 21.485a2 2 0 1 0 2.829-2.828l1.767 1.768a2 2 0 1 0 2.829-2.829l-6.364-6.364a2 2 0 1 0-2.829 2.829l1.768 1.767a2 2 0 0 0-2.828 2.829z",
    },
  ],
  ["path", { d: "m9.6 14.4 4.8-4.8" }],
];

const ACTIVITY: IconNode = [
  [
    "path",
    {
      d: "M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2",
    },
  ],
];

const BOOK_OPEN: IconNode = [
  ["path", { d: "M12 7v14" }],
  [
    "path",
    {
      d: "M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z",
    },
  ],
];

const CALENDAR: IconNode = [
  ["path", { d: "M8 2v4" }],
  ["path", { d: "M16 2v4" }],
  ["rect", { x: "3", y: "4", width: "18", height: "18", rx: "2" }],
  ["path", { d: "M3 10h18" }],
];

const ORBIT: IconNode = [
  ["circle", { cx: "12", cy: "12", r: "3" }],
  ["circle", { cx: "19", cy: "5", r: "2" }],
  ["circle", { cx: "5", cy: "19", r: "2" }],
  ["path", { d: "M10.4 21.6a10 10 0 0 1-8-8" }],
  ["path", { d: "M13.6 2.4a10 10 0 0 1 8 8" }],
  ["path", { d: "M21.6 13.6a10 10 0 0 1-8 8" }],
  ["path", { d: "M2.4 10.4a10 10 0 0 1 8-8" }],
];

const LIBRARY: IconNode = [
  ["path", { d: "m16 6 4 14" }],
  ["path", { d: "M12 6v14" }],
  ["path", { d: "M8 8v12" }],
  ["path", { d: "M4 4v16" }],
];

const FOLDER: IconNode = [
  [
    "path",
    {
      d: "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
    },
  ],
];

const SPARKLES: IconNode = [
  [
    "path",
    { d: "M9.94 14.5 8.5 18.5 7.06 14.5 3.06 13.06 7.06 11.62 8.5 7.62 9.94 11.62 13.94 13.06z" },
  ],
  ["path", { d: "M17.5 6.5 16.6 9 14.1 9.9 16.6 10.8 17.5 13.3 18.4 10.8 20.9 9.9 18.4 9z" }],
  ["path", { d: "M19 17.5 18.5 19 17 19.5 18.5 20 19 21.5 19.5 20 21 19.5 19.5 19z" }],
];

const USER: IconNode = [
  ["path", { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }],
  ["circle", { cx: "12", cy: "7", r: "4" }],
];

const ICON_MAP: Record<string, IconNode> = {
  document: FILE_TEXT,
  "document-text": FILE_TEXT,
  page: FILE,
  "game-controller": GAMEPAD_CONTROLLER,
  image: IMAGE,
  barbell: DUMBBELL,
  fitness: ACTIVITY,
  book: BOOK_OPEN,
  calendar: CALENDAR,
  planet: ORBIT,
  library: LIBRARY,
  folder: FOLDER,
  sparkles: SPARKLES,
  user: USER,
  person: USER,
};

function renderSvg(node: IconNode): string {
  const children = node
    .map(([tag, attrs]) => {
      const attrStr = Object.entries(attrs)
        .map(([k, v]) => `${k}="${v}"`)
        .join(" ");
      return `<${tag} ${attrStr}/>`;
    })
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${children}</svg>`;
}

const cache = new Map<string, string>();

/** Возвращает data:URI с Lucide-стилем SVG по имени anytype-icon'а. */
export function objectIconUri(name: string | null | undefined): string {
  const key = name && ICON_MAP[name] ? name : "document";
  const cached = cache.get(key);
  if (cached) return cached;
  const svg = renderSvg(ICON_MAP[key]);
  const uri = `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
  cache.set(key, uri);
  return uri;
}
