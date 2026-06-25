// Theme CSS is exported as `@kosmos/visuals/theme/css` from package.json.
// `theme/css-variables.css` is the single source of truth for runtime tokens.

// Components

export {
  type SidebarConfig,
  CommandPalette,
  GamePosterCard,
  SidebarButton,
  Sidebar,
  type SidebarNavItem,
  type SidebarProjectItem,
  type SidebarProjectGroup,
  Titlebar,
  type TitlebarPlatform,
  TitlebarHistoryControls,
  DesktopChrome,
  DesktopContentSurface,
  StatusDot,
  type StatusDotTone,
  TodoRow,
  type TodoRowItem,
  type TodoDropPayload,
  type TodoRowUpdate,
  QuickEntryPanel,
  type QuickEntryProject,
  type QuickEntrySavePayload,
  ContextMenu,
  ContextMenuItem,
  Modal,
  Calendar,
  DateChip,
  TimeColumn,
  DateTimePicker,
  Dropdown,
  IconButton,
  Toggle,
  Checkbox,
  SettingsRow,
  SettingsList,
  SettingsDropdownRow,
  SettingsToggleRow,
  SettingsButtonRow,
  SettingsTextInputRow,
  SettingsAdvancedIntro,
  SettingsSidebar,
  SettingsSidebarButton,
  SettingsContentHeader,
  SyncNodeRow,
  SettingsSearchInput,
  EmptyState,
  BlocklistCard,
  Toast,
  ToastHost,
  Button,
  TextInput,
  Textarea,
  RadioGroup,
  HotkeyCapture,
  KbdKey,
  ActionsPanel,
  Skeleton,
} from "./components";

// Composables

export { useContextMenu, type ContextMenuState } from "./composables/useContextMenu";

export {
  useToast,
  provideToastHost,
  type ToastOptions,
  type ToastTone,
  type ToastApi,
} from "./composables/useToast";

export { usePlatform, type PlatformInfo } from "./composables/usePlatform";

// Runtime helpers

export { installScrollFadeListener, type InstallScrollFadeOptions } from "./runtime/scroll-fade";
export {
  installConsoleOnlyRuntimeErrors,
  type ConsoleOnlyRuntimeErrorsOptions,
} from "./runtime/runtime-errors";
