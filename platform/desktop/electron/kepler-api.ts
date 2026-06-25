// Kepler extension API contract version.
//
// `KEPLER_API_VERSION` — semver-style версия того API, который Kepler shell
// предоставляет extension'ам через `window.kepler.*` preload bridge. Extension
// в своём `manifest.json` объявляет требуемый диапазон через
// `keplerApiVersion` (semver range, например `^1.0.0`). На load extension
// проверяется на совместимость: если range не удовлетворён — extension не
// загружается, пользователю показывается баннер «обновите расширение».
//
// Bump правила:
//   - patch (1.0.x): bugfix без изменения contract'а;
//   - minor (1.x.0): добавили новый method/event, старые работают;
//   - major (X.0.0): breaking change существующего contract'а.
//
// Manifest extension'а обычно объявляет `keplerApiVersion: "^1.0.0"` — это
// принимает любые 1.x.y версии shell'а, но отвергнет shell с major bump'ом.

export const KEPLER_API_VERSION = "1.1.0";

// ---------------------------------------------------------------------------
// Минимальный semver matcher: поддерживает то, что реально нужно для
// `keplerApiVersion` в manifest'ах. Полный node-semver не подключаем, чтобы
// не тащить лишнюю зависимость в Electron main process.
//
// Поддерживаемые форматы range:
//   - "1.2.3"            — exact match
//   - "^1.2.3"           — caret: same major (для 0.x — same minor)
//   - "~1.2.3"           — tilde: same major+minor
//   - ">=1.2.3"          — minimum
//   - ">1.2.3"           — strict greater
//   - "<=1.2.3" / "<1.2.3"
//   - "*"                — any
//   - "1.2.3 - 2.0.0"    — range (inclusive)
//   - "1.x" / "1.2.x"    — wildcard
//   - " || "             — OR-комбинация range'ей
//
// Не поддерживается: prerelease tags (`-beta.1`), `build` metadata.
// ---------------------------------------------------------------------------

interface SemVer {
  major: number;
  minor: number;
  patch: number;
}

function parseSemver(v: string): SemVer | null {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(v.trim());
  if (!m) return null;
  return {
    major: parseInt(m[1]!, 10),
    minor: parseInt(m[2]!, 10),
    patch: parseInt(m[3]!, 10),
  };
}

function cmp(a: SemVer, b: SemVer): number {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}

function expandWildcard(v: string): { min: SemVer; max: SemVer } | null {
  // "1" → 1.0.0 .. <2.0.0; "1.2" / "1.2.x" → 1.2.0 .. <1.3.0
  const parts = v.split(".");
  if (parts.length === 0) return null;
  const nums: number[] = [];
  let foundWildcard = false;
  for (const p of parts) {
    if (p === "x" || p === "X" || p === "*") {
      foundWildcard = true;
      break;
    }
    const n = parseInt(p, 10);
    if (Number.isNaN(n)) return null;
    nums.push(n);
  }
  if (!foundWildcard && nums.length === 3) {
    // exact 1.2.3 — special case
    return null;
  }
  const major = nums[0] ?? 0;
  const minor = nums[1] ?? 0;
  if (nums.length === 0) {
    return {
      min: { major: 0, minor: 0, patch: 0 },
      max: { major: Number.MAX_SAFE_INTEGER, minor: 0, patch: 0 },
    };
  }
  if (nums.length === 1) {
    return {
      min: { major, minor: 0, patch: 0 },
      max: { major: major + 1, minor: 0, patch: 0 },
    };
  }
  // length === 2
  return {
    min: { major, minor, patch: 0 },
    max: { major, minor: minor + 1, patch: 0 },
  };
}

function matchSingle(range: string, version: SemVer): boolean {
  const r = range.trim();
  if (r === "" || r === "*") return true;

  // hyphen range "1.2.3 - 2.0.0"
  const hyphen = r.split(/\s+-\s+/);
  if (hyphen.length === 2) {
    const lo = parseSemver(hyphen[0]!);
    const hi = parseSemver(hyphen[1]!);
    if (!lo || !hi) return false;
    return cmp(version, lo) >= 0 && cmp(version, hi) <= 0;
  }

  // caret ^1.2.3
  if (r.startsWith("^")) {
    const base = parseSemver(r.slice(1));
    if (!base) return false;
    if (cmp(version, base) < 0) return false;
    // 0.x.y → only same minor; 0.0.x → only same patch
    if (base.major === 0) {
      if (base.minor === 0) {
        return version.major === 0 && version.minor === 0 && version.patch === base.patch;
      }
      return version.major === 0 && version.minor === base.minor;
    }
    return version.major === base.major;
  }

  // tilde ~1.2.3
  if (r.startsWith("~")) {
    const base = parseSemver(r.slice(1));
    if (!base) return false;
    if (cmp(version, base) < 0) return false;
    return version.major === base.major && version.minor === base.minor;
  }

  // comparators: >=, <=, >, <, =
  const cmpMatch = /^(>=|<=|>|<|=)\s*(.+)$/.exec(r);
  if (cmpMatch) {
    const op = cmpMatch[1]!;
    const base = parseSemver(cmpMatch[2]!);
    if (!base) return false;
    const c = cmp(version, base);
    switch (op) {
      case ">=":
        return c >= 0;
      case "<=":
        return c <= 0;
      case ">":
        return c > 0;
      case "<":
        return c < 0;
      case "=":
        return c === 0;
    }
  }

  // exact "1.2.3"
  const exact = parseSemver(r);
  if (exact) return cmp(version, exact) === 0;

  // wildcards "1.x" / "1.2.x" / "1"
  const wc = expandWildcard(r);
  if (wc) {
    return cmp(version, wc.min) >= 0 && cmp(version, wc.max) < 0;
  }

  return false;
}

/**
 * Проверка совместимости версии с range. Range может содержать `||` для
 * OR-комбинации. Возвращает true если version удовлетворяет хотя бы одной
 * ветви range.
 */
export function satisfiesSemver(version: string, range: string): boolean {
  const v = parseSemver(version);
  if (!v) return false;
  const branches = range
    .split("||")
    .map((s) => s.trim())
    .filter(Boolean);
  if (branches.length === 0) return matchSingle(range, v);
  return branches.some((b) => matchSingle(b, v));
}
