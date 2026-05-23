export const colors = {
  light: {
    background: "oklch(1 0 0)",

    foreground: "oklch(0.145 0 0)",

    card: "oklch(1 0 0)",

    cardForeground: "oklch(0.145 0 0)",

    popover: "oklch(1 0 0)",

    popoverForeground: "oklch(0.145 0 0)",

    primary: "oklch(0.205 0 0)",

    primaryForeground: "oklch(0.985 0 0)",

    secondary: "oklch(0.97 0 0)",

    secondaryForeground: "oklch(0.205 0 0)",

    muted: "oklch(0.97 0 0)",

    mutedForeground: "oklch(0.556 0 0)",

    accent: "oklch(0.97 0 0)",

    accentForeground: "oklch(0.205 0 0)",

    destructive: "oklch(0.577 0.245 27.325)",

    border: "oklch(0.922 0 0)",

    input: "oklch(0.922 0 0)",

    ring: "oklch(0.708 0 0)",

    sidebar: "var(--background)",

    sidebarForeground: "oklch(0.145 0 0)",

    sidebarPrimary: "oklch(0.205 0 0)",

    sidebarPrimaryForeground: "oklch(0.985 0 0)",

    sidebarAccent: "oklch(0.97 0 0)",

    sidebarAccentForeground: "oklch(0.205 0 0)",

    sidebarBorder: "oklch(0.922 0 0)",

    sidebarRing: "oklch(0.708 0 0)",

    mainBackground: "rgb(13 13 13 / 90%)",

    secondText: "#717171",

    settingsBorder: "#313131",

    settingsSearchSurface: "#262626",

    settingsSearchSurfaceFocused: "#303030",

    settingsSidebarActive: "#343434",

    settingsSidebarIconFrom: "oklch(0.42 0 0)",

    settingsSidebarIconTo: "oklch(0.26 0 0)",

    settingsListBackground: "#222222",
  },

  dark: {
    background: "oklch(22.213% 0.00003 271.152)",

    foreground: "oklch(0.985 0 0)",

    card: "oklch(0.205 0 0)",

    cardForeground: "oklch(0.985 0 0)",

    popover: "oklch(0.205 0 0)",

    popoverForeground: "oklch(0.985 0 0)",

    primary: "oklch(0.922 0 0)",

    primaryForeground: "oklch(0.205 0 0)",

    secondary: "oklch(26.862% 0.00003 271.152)",

    secondaryForeground: "oklch(0.985 0 0)",

    muted: "oklch(0.269 0 0)",

    mutedForeground: "oklch(0.708 0 0)",

    accent: "oklch(0.269 0 0)",

    accentForeground: "oklch(0.985 0 0)",

    destructive: "oklch(0.704 0.191 22.216)",

    border: "oklch(32.897% 0.00004 271.152)",

    input: "oklch(1 0 0 / 15%)",

    ring: "#797979",

    sidebar: "var(--background)",

    sidebarForeground: "oklch(0.985 0 0)",

    sidebarPrimary: "oklch(0.58 0 none)",

    sidebarPrimaryForeground: "oklch(0.985 0 0)",

    sidebarAccent: "oklch(0.32 0 none)",

    sidebarAccentForeground: "oklch(0.985 0 0)",

    sidebarBorder: "oklch(1 0 0 / 10%)",

    sidebarRing: "oklch(0.556 0 0)",

    mainBackground: "rgb(13 13 13 / 90%)",

    secondText: "#717171",

    settingsBorder: "#313131",

    settingsSearchSurface: "#262626",

    settingsSearchSurfaceFocused: "#303030",

    settingsSidebarActive: "#343434",

    settingsSidebarIconFrom: "oklch(0.42 0 0)",

    settingsSidebarIconTo: "oklch(0.26 0 0)",

    settingsListBackground: "#222222",
  },

  status: {
    success: "#10b981",

    warning: "#f59e0b",

    error: "#ef4444",

    info: "#3b82f6",
  },

  smartList: {
    inbox: "#3b82f6",

    today: "#eab308",

    upcoming: "#ef4444",

    anytime: "#a855f7",

    someday: "#92400e",

    logbook: "#22c55e",

    trash: "#6b7280",
  },
} as const;
