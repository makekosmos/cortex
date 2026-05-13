#!/usr/bin/env node
// Read-only inspection of ARK SQLite databases.
//
// Usage:
//   node scripts/ark-inspect.mjs                     # default user DB
//   node scripts/ark-inspect.mjs <path-to-db>
//   node scripts/ark-inspect.mjs --all               # iterate all space DBs
//
// Prints: table list with row counts, breakdown of objects by type_id,
// sample rows per object type, last few usage sessions.

import { DatabaseSync } from "node:sqlite";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";

const APPDATA = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
const DEFAULT_DB = path.join(APPDATA, "Kepler", "ark.db");
const SPACES_DIR = path.join(APPDATA, "Kepler", "spaces");

function inspect(dbPath) {
  if (!fs.existsSync(dbPath)) {
    console.log(`  (no file at ${dbPath})\n`);
    return;
  }

  const stat = fs.statSync(dbPath);
  console.log(`\n=== ${dbPath} ===`);
  console.log(`  size: ${(stat.size / 1024).toFixed(1)} KB · modified: ${stat.mtime.toISOString()}`);

  const db = new DatabaseSync(dbPath, { readOnly: true });

  // tables and row counts
  const tables = db
    .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
    .all();

  console.log("\n  tables:");
  for (const { name } of tables) {
    const count = db.prepare(`SELECT COUNT(*) AS n FROM "${name}"`).get().n;
    console.log(`    ${name.padEnd(28)} ${String(count).padStart(8)}`);
  }

  // objects breakdown by type
  if (tables.find(t => t.name === "objects")) {
    const byType = db
      .prepare("SELECT type_id, COUNT(*) AS n FROM objects WHERE deleted_at IS NULL GROUP BY type_id ORDER BY n DESC")
      .all();
    if (byType.length > 0) {
      console.log("\n  objects by type:");
      for (const { type_id, n } of byType) {
        console.log(`    ${String(type_id).padEnd(28)} ${String(n).padStart(8)}`);
      }
    }

    // sample object per type
    for (const { type_id } of byType) {
      const sample = db
        .prepare(`SELECT id, title, substr(content_json, 1, 80) AS content, substr(props_json, 1, 80) AS props, created_at FROM objects WHERE type_id = ? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT 1`)
        .get(type_id);
      if (sample) {
        console.log(`\n  sample ${type_id}:`);
        console.log(`    id:      ${sample.id}`);
        console.log(`    title:   ${sample.title ?? "—"}`);
        console.log(`    content: ${sample.content ?? "—"}`);
        console.log(`    props:   ${sample.props ?? "—"}`);
        console.log(`    created: ${sample.created_at}`);
      }
    }
  }

  // tracked_apps
  if (tables.find(t => t.name === "tracked_apps")) {
    const apps = db.prepare("SELECT COUNT(*) AS n FROM tracked_apps").get().n;
    if (apps > 0) {
      const top = db
        .prepare("SELECT display_name, process_name, last_seen_at FROM tracked_apps ORDER BY last_seen_at DESC LIMIT 5")
        .all();
      console.log("\n  recent tracked apps:");
      for (const a of top) {
        console.log(`    ${(a.display_name || a.process_name || "—").padEnd(40)} last_seen ${a.last_seen_at}`);
      }
    }
  }

  // usage_sessions
  if (tables.find(t => t.name === "usage_sessions")) {
    const total = db.prepare("SELECT COUNT(*) AS n, SUM(foreground_ms) AS ms FROM usage_sessions").get();
    if (total.n > 0) {
      const totalH = ((total.ms || 0) / 1000 / 60 / 60).toFixed(1);
      console.log(`\n  usage_sessions: ${total.n} sessions, total foreground ${totalH}h`);
      const latest = db
        .prepare("SELECT started_at, ended_at, foreground_ms, idle_ms, process_name FROM usage_sessions ORDER BY started_at DESC LIMIT 5")
        .all();
      for (const s of latest) {
        const min = (s.foreground_ms / 1000 / 60).toFixed(1);
        console.log(`    ${s.started_at}  fg=${min}min  ${s.process_name}`);
      }
    }
  }

  // sync_tombstones (record of deletes for propagation to peers)
  if (tables.find(t => t.name === "sync_tombstones")) {
    const tombs = db.prepare("SELECT id, entity_type, deleted_at FROM sync_tombstones ORDER BY deleted_at DESC LIMIT 5").all();
    if (tombs.length > 0) {
      console.log("\n  recent tombstones:");
      for (const t of tombs) {
        console.log(`    ${t.entity_type.padEnd(20)} ${t.id.padEnd(40)} ${t.deleted_at}`);
      }
    }
  }

  // sync_kv (version vector etc)
  if (tables.find(t => t.name === "sync_kv")) {
    const rows = db.prepare("SELECT key, substr(value, 1, 120) AS value FROM sync_kv ORDER BY key").all();
    if (rows.length > 0) {
      console.log("\n  sync_kv:");
      for (const r of rows) {
        console.log(`    ${r.key}: ${r.value}`);
      }
    }
  }

  db.close();
}

const args = process.argv.slice(2);

if (args[0] === "--all") {
  inspect(DEFAULT_DB);
  if (fs.existsSync(SPACES_DIR)) {
    for (const entry of fs.readdirSync(SPACES_DIR)) {
      const p = path.join(SPACES_DIR, entry, "ark.db");
      if (fs.existsSync(p)) inspect(p);
    }
  }
} else if (args[0]) {
  inspect(args[0]);
} else {
  inspect(DEFAULT_DB);
}
