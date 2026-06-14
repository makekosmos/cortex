/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";

  const component: DefineComponent<Record<string, unknown>, Record<string, unknown>, unknown>;

  export default component;
}

interface Entry {
  id: string;

  title: string;

  content_json: string;

  created_at: number;

  updated_at: number;

  folder_id: string | null;

  type_id: string | null;

  header_layout: string | null;

  header_props_json: string;

  schema_version: number;

  deleted_at: number | null;
}

type NoteFieldKind =
  | "text"
  | "long_text"
  | "number"
  | "date"
  | "boolean"
  | "select"
  | "multi_select"
  | "image"
  | "relation";

type HeaderLayoutKind = "default" | "centered_profile";

interface NoteTypeField {
  id: string;

  label: string;

  kind: NoteFieldKind;

  required: boolean;

  options?: string[];

  placeholder?: string;

  visible?: boolean;

  read_only?: boolean;

  link_type?: string;
}

interface HeaderTemplateDefinition {
  kind: HeaderLayoutKind;

  primaryFieldIds?: string[];

  secondaryFieldIds?: string[];

  imageFieldId?: string | null;
}

interface NoteType {
  id: string;

  name: string;

  slug: string;

  icon: string | null;

  color: string | null;

  schema_json: string;

  header_template_json: string;

  ui_schema_json?: string;

  created_at: number;

  updated_at: number;
}

interface NoteTypeUiSchema {
  featured_fields?: string[];
  visible_fields?: string[];
  hidden_fields?: string[];
  read_only_fields?: string[];
  field_order?: string[];
  header_layout?: "inline" | "column";
  default_layout?: "page" | "list" | "gallery" | "board";
  default_template_id?: string | null;
  collection_name?: string;
}

interface SearchResult {
  file: string;

  line: number;

  text: string;

  entryId: string;
}

interface Folder {
  id: string;

  name: string;

  created_at: number;

  parent_id: string | null;
}

type CreateFolderResult =
  | {
      ok: true;

      folder: Folder;
    }
  | {
      ok: false;

      reason: "duplicate_folder_name";

      name: string;
    };

type MoveFolderResult =
  | {
      ok: true;

      folderId: string;

      parent_id: string | null;
    }
  | {
      ok: false;

      reason: "folder_not_found" | "invalid_target" | "duplicate_folder_name";

      message: string;
    };

type SaveEntryResult =
  | {
      ok: true;

      entryId: string;
    }
  | {
      ok: false;

      reason: "duplicate_title";

      conflictingEntryId: string;

      title: string;

      folder_id: string | null;
    }
  | {
      ok: false;

      reason: "invalid_type_metadata";

      message: string;
    };

type DeleteEntryResult =
  | {
      ok: true;

      entryId: string;
    }
  | {
      ok: false;

      reason: "entry_not_found";

      message: string;
    };

type DeleteFolderResult =
  | {
      ok: true;

      folderId: string;
    }
  | {
      ok: false;

      reason: "folder_not_found";

      message: string;
    }
  | {
      ok: false;

      reason: "folder_not_empty";

      message: string;

      entryCount: number;
    };

type SaveNoteTypeResult =
  | {
      ok: true;

      noteType: NoteType;
    }
  | {
      ok: false;

      reason: "duplicate_slug" | "invalid_definition";

      message: string;
    };

interface ExportMarkdownVaultResult {
  ok: boolean;

  exportedCount: number;

  outputDir: string;
}

interface MarkdownFileOpenResult {
  path: string;

  name: string;

  content: string;
}

interface MarkdownFileSaveResult {
  path: string;
}

interface MarkdownVaultTextFile {
  path: string;

  relativePath: string;

  name: string;

  content: string;
}

interface MarkdownVaultImageFile {
  path: string;

  relativePath: string;

  name: string;

  fileUrl: string;

  mimeType: string;

  sizeBytes: number;

  width: number | null;

  height: number | null;
}

interface MarkdownVaultOpenResult {
  rootPath: string;

  files: MarkdownVaultTextFile[];

  images: MarkdownVaultImageFile[];
}

interface MarkdownVaultExportFile {
  relativePath: string;

  content?: string;

  sourcePath?: string;
}

interface MarkdownVaultExportResult {
  outputDir: string;

  exportedCount: number;
}

interface VaultStorageInfo {
  textBytes: number;

  trashBytes: number;

  dbBytes: number;

  vaultBytes: number;

  entryCount: number;

  trashCount: number;
}

interface EdenPerfMetricSummary {
  count: number;

  minMs: number;

  maxMs: number;

  avgMs: number;

  p50Ms: number;

  p95Ms: number;

  p99Ms: number;
}

interface EdenPerfSummary {
  inputToNextPaint: EdenPerfMetricSummary;

  updateToNextPaint: EdenPerfMetricSummary;

  saveDuration: EdenPerfMetricSummary;

  longTaskCount: number;

  longTaskMaxDurationMs: number;
}

interface EdenPerfTracker {
  reset: () => void;

  getSummary: () => EdenPerfSummary;
}

interface Window {
  api: {
    saveEntry: (entry: Entry) => Promise<SaveEntryResult>;

    loadEntry: (id: string) => Promise<Entry | undefined>;

    listEntries: () => Promise<Entry[]>;

    getVaultPath: () => Promise<string | null>;

    getRecentVaultPaths: () => Promise<string[]>;

    selectFolder: () => Promise<string | null>;

    setVaultPath: (path: string) => Promise<boolean>;

    openMarkdownFile: () => Promise<MarkdownFileOpenResult | null>;

    saveMarkdownFile: (
      suggestedName: string,

      content: string,
    ) => Promise<MarkdownFileSaveResult | null>;

    openMarkdownVault: () => Promise<MarkdownVaultOpenResult | null>;

    exportMarkdownVault: (
      files?: MarkdownVaultExportFile[],
    ) => Promise<MarkdownVaultExportResult | ExportMarkdownVaultResult | null>;

    searchEntries: (query: string) => Promise<SearchResult[]>;

    createFolder: (
      id: string,

      name: string,

      parentId?: string | null,
    ) => Promise<CreateFolderResult>;

    listFolders: () => Promise<Folder[]>;

    listNoteTypes: () => Promise<NoteType[]>;

    saveNoteType: (noteType: NoteType) => Promise<SaveNoteTypeResult>;

    deleteNoteType: (noteTypeId: string) => Promise<boolean>;

    moveEntryToFolder: (entryId: string, folderId: string | null) => Promise<boolean>;

    moveFolderToFolder: (folderId: string, parentId: string | null) => Promise<MoveFolderResult>;

    deleteEntry: (entryId: string) => Promise<DeleteEntryResult>;

    deleteFolder: (folderId: string) => Promise<DeleteFolderResult>;

    getSidebarConfig: () => Promise<{
      widget: { width: number; hidden: boolean };
    }>;

    updateSidebarConfig: (config: { widget?: { width?: number; hidden?: boolean } }) => Promise<{
      widget: { width: number; hidden: boolean };
    }>;

    getEdenVisibleObjectTypeIds: () => Promise<string[]>;

    setEdenVisibleObjectTypeIds: (typeIds: string[]) => Promise<string[]>;

    getPlatform: () => Promise<NodeJS.Platform>;

    listTrashEntries: () => Promise<Entry[]>;

    restoreEntry: (entryId: string) => Promise<{ ok: boolean; entryId?: string }>;

    permanentDeleteEntry: (entryId: string) => Promise<{ ok: boolean; entryId?: string }>;

    purgeExpiredTrash: () => Promise<{ ok: boolean; purgedCount: number }>;

    getVaultStorageInfo: () => Promise<VaultStorageInfo>;

    getDiskFreeSpace: () => Promise<number>;

    zoomGet: () => Promise<number>;

    zoomSet: (factor: number) => Promise<number>;

    minimize: () => void;

    maximize: () => void;

    close: () => void;

    onCommand: (
      channel: "eden:cmd:note:create" | "eden:cmd:note:search",
      handler: (params: unknown) => void,
    ) => () => void;
  };

  __edenPerf?: EdenPerfTracker;

  // Kepler shared preload — exposed когда extension загружен Kepler shell'ом.
  // Может быть undefined если страница открыта вне Kepler (например plain
  // vite dev server).
  kepler?: {
    ark: {
      request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
      subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
    };
    window?: {
      close: () => void;
      minimize: () => void;
      maximize: () => void;
      zoomGet?: () => Promise<number>;
      zoomSet?: (factor: number) => Promise<number>;
      setMaximizable?: (value: boolean) => Promise<void>;
      setTitlebarSymbolColor?: (symbolColor: string) => Promise<void>;
      beginManualDrag?: (point: { screenX: number; screenY: number }) => Promise<void>;
      moveManualDrag?: (point: { screenX: number; screenY: number }) => Promise<void>;
      endManualDrag?: () => Promise<void>;
      setTitlebarHoverTracking?: (enabled: boolean, height: number) => Promise<void>;
      onTitlebarHoverChange?: (handler: (hovered: boolean) => void) => () => void;
    };
    navigation?: {
      initialRoute: () => Promise<string | null>;
      onNavigate: (handler: (route: string) => void) => () => void;
    };
    meta?: {
      id: () => Promise<string>;
    };
    userData?: {
      readJson: <T = unknown>(name: string) => Promise<T | null>;
      writeJson: (name: string, value: unknown) => Promise<void>;
      readFile: (name: string) => Promise<string | null>;
      writeFile: (name: string, content: string) => Promise<void>;
      path: () => Promise<string>;
    };
    markdownFiles?: {
      open: () => Promise<MarkdownFileOpenResult | null>;
      save: (suggestedName: string, content: string) => Promise<MarkdownFileSaveResult | null>;
      openVault: () => Promise<MarkdownVaultOpenResult | null>;
      exportVault: (files: MarkdownVaultExportFile[]) => Promise<MarkdownVaultExportResult | null>;
    };
  };
}
