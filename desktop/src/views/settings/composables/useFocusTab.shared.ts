export interface FocusBlocklist {
  id: string;
  name: string;
  domains: string[];
  createdAt: string;
  preset?: boolean;
  icon?: string;
  kind?: "domains" | "raw";
}

export interface FocusActiveState {
  active: boolean;
  blocklist_id?: string | null;
  started_at?: string | null;
}

export const ICON_CHOICES = [
  "🛡️",
  "🚫",
  "🎮",
  "🧠",
  "📰",
  "📺",
  "🎬",
  "💬",
  "🐦",
  "📷",
  "🛒",
  "⚽",
  "🎰",
  "🍔",
  "💸",
  "🎵",
  "📚",
  "⚙️",
  "🔒",
  "🎯",
  "⏰",
  "🌐",
  "✨",
  "🔥",
  "⚡",
] as const;

const DOMAIN_PATTERN = /^[a-z0-9][a-z0-9.-]*\.[a-z]{2,}$/i;

export interface FocusDraftParsed {
  domains: string[];
  kind: "domains" | "raw";
  invalid: string[];
}

export function isUnknownOperationError<T>(err: T): boolean {
// SAFETY: the surrounding domain validation preserves the asserted contract.
  const msg = (err as Error)?.message ?? String(err);
  return /unknown operation|unknown_operation|not.?found/i.test(msg);
}

export function parseFocusDraft(raw: string): FocusDraftParsed {
  const lines = raw.split("\n");
  const valid: string[] = [];
  const invalid: string[] = [];
  let hasReferences = false;

  for (const line of lines) {
    const stripped = line.replace(/#.*$/, "").trim();
    if (!stripped) continue;

    if (stripped.startsWith("@")) {
      const refId = stripped.slice(1).trim();
      if (!refId) {
        invalid.push(stripped);
        continue;
      }
      hasReferences = true;
      valid.push(`@${refId}`);
      continue;
    }

    if (DOMAIN_PATTERN.test(stripped)) {
      valid.push(stripped.toLowerCase());
      continue;
    }

    invalid.push(stripped);
  }

  return {
    domains: valid,
    kind: hasReferences ? "raw" : "domains",
    invalid,
  };
}
