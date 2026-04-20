import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { Database } from "bun:sqlite";

function parseArg(name: string) {
  const flag = `--${name}`;
  const index = process.argv.indexOf(flag);
  return index === -1 ? undefined : process.argv[index + 1];
}

function iso(timestamp: number) {
  return new Date(timestamp).toISOString();
}

function demoDoc(text: string) {
  return {
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: [{ type: "text", text }],
      },
    ],
  };
}

const CREATE_OBJECT_SCHEMA_SQL = `
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS object_types (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schema_json TEXT NOT NULL,
    ui_schema_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    system_locked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS objects (
    id TEXT PRIMARY KEY,
    type_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content_json TEXT NOT NULL DEFAULT '{}',
    props_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY(type_id) REFERENCES object_types(id)
);

CREATE TABLE IF NOT EXISTS object_links (
    id TEXT PRIMARY KEY,
    source_object_id TEXT NOT NULL,
    target_object_id TEXT NOT NULL,
    link_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(source_object_id) REFERENCES objects(id) ON DELETE CASCADE,
    FOREIGN KEY(target_object_id) REFERENCES objects(id) ON DELETE CASCADE,
    UNIQUE(source_object_id, target_object_id, link_type)
);

CREATE INDEX IF NOT EXISTS idx_objects_type_id ON objects(type_id);
CREATE INDEX IF NOT EXISTS idx_objects_updated_at ON objects(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_objects_deleted_at ON objects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_object_links_source ON object_links(source_object_id);
CREATE INDEX IF NOT EXISTS idx_object_links_target ON object_links(target_object_id);
`;

const builtinNoteType = {
  id: "note_obj",
  name: "Заметка",
  schema_json: JSON.stringify({
    fields: [
      {
        id: "description",
        label: "Описание",
        kind: "long_text",
        required: false,
        visible: true,
        read_only: false,
        system: false,
      },
      {
        id: "related_notes",
        label: "Связанные заметки",
        kind: "relation",
        required: false,
        visible: true,
        read_only: false,
        link_type: "related",
        system: false,
      },
    ],
  }),
  ui_schema_json: JSON.stringify({
    featured_fields: ["description"],
    visible_fields: ["description", "related_notes"],
    hidden_fields: ["created_at", "updated_at", "deleted_at"],
    read_only_fields: [],
    field_order: ["description", "related_notes"],
    header_layout: "inline",
    default_layout: "page",
    default_template_id: null,
  }),
};

const builtinGameType = {
  id: "game_obj",
  name: "Игра",
  schema_json: JSON.stringify({
    fields: [
      { id: "description", label: "Описание", kind: "long_text", required: false, visible: true, read_only: false, system: false },
      { id: "user_rating", label: "Оценка", kind: "number", required: false, visible: true, read_only: false, system: false },
      {
        id: "play_status",
        label: "Статус",
        kind: "select",
        required: false,
        visible: true,
        read_only: false,
        options: ["not_started", "in_progress", "completed", "abandoned"],
        system: false,
      },
      { id: "genres", label: "Жанры", kind: "text", required: false, visible: true, read_only: false, system: false },
      { id: "cover_image", label: "Обложка", kind: "image", required: false, visible: true, read_only: false, system: false },
      { id: "background_image", label: "Фон", kind: "image", required: false, visible: true, read_only: false, system: false },
      {
        id: "related_notes",
        label: "Связанные заметки",
        kind: "relation",
        required: false,
        visible: true,
        read_only: false,
        link_type: "related",
        system: false,
      },
      { id: "exe_path", label: "Путь к игре", kind: "text", required: false, visible: true, read_only: false, system: true },
      { id: "save_path", label: "Путь к сейвам", kind: "text", required: false, visible: true, read_only: false, system: true },
      { id: "total_playtime_seconds", label: "Время игры", kind: "number", required: false, visible: true, read_only: true, system: true },
      { id: "last_played_at", label: "Последний запуск", kind: "date", required: false, visible: true, read_only: true, system: true },
      { id: "play_count", label: "Запусков", kind: "number", required: false, visible: true, read_only: true, system: true },
      { id: "save_exists", label: "Сейв найден", kind: "boolean", required: false, visible: true, read_only: true, system: true },
      { id: "rawg_id", label: "RAWG ID", kind: "text", required: false, visible: false, read_only: true, system: true },
      { id: "exe_name", label: "Имя exe", kind: "text", required: false, visible: false, read_only: true, system: true },
    ],
  }),
  ui_schema_json: JSON.stringify({
    featured_fields: [
      "play_status",
      "genres",
      "user_rating",
      "total_playtime_seconds",
      "last_played_at",
      "play_count",
      "save_exists",
    ],
    visible_fields: [
      "description",
      "play_status",
      "genres",
      "user_rating",
      "cover_image",
      "background_image",
      "related_notes",
      "exe_path",
      "save_path",
      "total_playtime_seconds",
      "last_played_at",
      "play_count",
      "save_exists",
    ],
    hidden_fields: ["created_at", "updated_at", "deleted_at", "rawg_id", "exe_name", "sync_source"],
    read_only_fields: ["total_playtime_seconds", "last_played_at", "play_count", "save_exists", "rawg_id", "exe_name"],
    field_order: [
      "description",
      "play_status",
      "genres",
      "user_rating",
      "cover_image",
      "background_image",
      "related_notes",
      "exe_path",
      "save_path",
      "total_playtime_seconds",
      "last_played_at",
      "play_count",
      "save_exists",
      "rawg_id",
      "exe_name",
    ],
    header_layout: "column",
    default_layout: "page",
    default_template_id: null,
  }),
};

const dbPath = path.resolve(
  parseArg("db") ?? path.join(import.meta.dir, "..", "dev-data", "ark-demo-mixed.db"),
);

fs.mkdirSync(path.dirname(dbPath), { recursive: true });
if (fs.existsSync(dbPath)) {
  fs.rmSync(dbPath, { force: true });
}

const db = new Database(dbPath, { create: true });
db.exec(CREATE_OBJECT_SCHEMA_SQL);

const insertObjectType = db.prepare(`
  INSERT INTO object_types (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
  VALUES (?, ?, ?, ?, ?, ?, ?)
`);

const insertObject = db.prepare(`
  INSERT INTO objects (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
  VALUES (?, ?, ?, ?, ?, ?, ?, ?)
`);

const insertObjectLink = db.prepare(`
  INSERT INTO object_links (id, source_object_id, target_object_id, link_type, created_at)
  VALUES (?, ?, ?, ?, ?)
`);

const now = Date.now();
const nowIso = iso(now);

const noteId = randomUUID();
const gameId = randomUUID();
const bookTypeId = "book_obj";
const bookId = randomUUID();

const transaction = db.transaction(() => {
  insertObjectType.run(
    builtinNoteType.id,
    builtinNoteType.name,
    builtinNoteType.schema_json,
    builtinNoteType.ui_schema_json,
    "1970-01-01T00:00:00.000Z",
    "1970-01-01T00:00:00.000Z",
    1,
  );

  insertObjectType.run(
    builtinGameType.id,
    builtinGameType.name,
    builtinGameType.schema_json,
    builtinGameType.ui_schema_json,
    "1970-01-01T00:00:00.000Z",
    "1970-01-01T00:00:00.000Z",
    1,
  );

  insertObjectType.run(
    bookTypeId,
    "Книга",
    JSON.stringify({
      fields: [
        { id: "description", label: "Описание", kind: "long_text", required: false, visible: true, read_only: false, system: false },
        { id: "author", label: "Автор", kind: "text", required: false, visible: true, read_only: false, system: false },
        {
          id: "status",
          label: "Статус",
          kind: "select",
          required: false,
          visible: true,
          read_only: false,
          system: false,
          options: ["planned", "reading", "done"],
        },
      ],
    }),
    JSON.stringify({
      featured_fields: ["author", "status"],
      visible_fields: ["description", "author", "status"],
      hidden_fields: ["created_at", "updated_at", "deleted_at"],
      read_only_fields: [],
      field_order: ["description", "author", "status"],
      header_layout: "inline",
      default_layout: "page",
      default_template_id: null,
    }),
    nowIso,
    nowIso,
    0,
  );

  insertObject.run(
    noteId,
    "note_obj",
    "Eden progress note",
    JSON.stringify(demoDoc("Regular note object for note_obj rendering and relation checks.")),
    JSON.stringify({
      description: "Normal note object linked from game and custom type demos.",
    }),
    iso(now - 1000 * 60 * 60 * 24),
    iso(now - 1000 * 60 * 12),
    null,
  );

  insertObject.run(
    gameId,
    "game_obj",
    "Cyberpunk 2077",
    JSON.stringify(demoDoc("Game note with cover, background, and usage-derived fields.")),
    JSON.stringify({
      description: "Night City, rich color, and enough metadata to exercise the new header.",
      user_rating: "9",
      play_status: "in_progress",
      genres: "RPG, Action",
      cover_image: "https://images.igdb.com/igdb/image/upload/t_cover_big/co7497.webp",
      background_image: "https://images.igdb.com/igdb/image/upload/t_screenshot_big/sc7o8f.webp",
      exe_path: "D:\\Games\\Cyberpunk 2077\\bin\\x64\\Cyberpunk2077.exe",
      save_path: "D:\\Users\\Player\\Saved Games\\CD Projekt Red\\Cyberpunk 2077",
      total_playtime_seconds: "48210",
      last_played_at: "2026-04-19",
      play_count: "24",
      save_exists: true,
      rawg_id: "41494",
      exe_name: "Cyberpunk2077.exe",
    }),
    iso(now - 1000 * 60 * 60 * 24 * 7),
    iso(now - 1000 * 60 * 30),
    null,
  );

  insertObject.run(
    bookId,
    bookTypeId,
    "The Left Hand of Darkness",
    JSON.stringify(demoDoc("Custom object type demo for object settings and preview testing.")),
    JSON.stringify({
      description: "Custom type included so the object-type library is not limited to built-ins.",
      author: "Ursula K. Le Guin",
      status: "reading",
    }),
    iso(now - 1000 * 60 * 60 * 48),
    iso(now - 1000 * 60 * 90),
    null,
  );

  insertObjectLink.run(
    `${gameId}:related:${noteId}`,
    gameId,
    noteId,
    "related",
    iso(now - 1000 * 60 * 25),
  );

  insertObjectLink.run(
    `${bookId}:related:${noteId}`,
    bookId,
    noteId,
    "related",
    iso(now - 1000 * 60 * 20),
  );
});

transaction();

const objectTypeIds = db
  .query("SELECT id FROM object_types ORDER BY id")
  .all() as Array<{ id: string }>;

const objects = db
  .query("SELECT id, type_id as typeId, title FROM objects ORDER BY updated_at DESC")
  .all() as Array<{ id: string; typeId: string; title: string }>;

db.close();

console.log(
  JSON.stringify(
    {
      dbPath,
      objectTypeIds: objectTypeIds.map((item) => item.id),
      objects,
    },
    null,
    2,
  ),
);
