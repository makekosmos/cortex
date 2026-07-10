import { readEntryTiptapDoc, writeEntryTiptapDoc } from "../editor-content/content";
import type { JSONContent } from "@tiptap/core";
import {
  bubbleOccurrenceMillis,
  bubblePlainText,
  formatBubbleDateKey,
  normalizeBubbleKind,
  normalizeBubbleThreads,
  parseBubbleDraft,
  plainTextToTiptapDoc,
  type BubbleKind,
  type BubbleTimelineNode,
} from "../components/bubbles/bubbleDiaryModel";
import { SYSTEM_TYPE_JOURNAL_ID } from "./systemTypeDefinitions";
import type { ArkObjectLinkRecord, ArkObjectRecord } from "./kepler-entry-mappers";

type ArkRequest = <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;

const REPLY_LINK_TYPE = "reply_to";

function ensureList<T>(value: unknown): T[] {
  if (Array.isArray(value)) return value as T[];
  if (value && typeof value === "object") {
    const record = value as Record<string, unknown>;
    for (const key of ["items", "objects", "links"]) {
      if (Array.isArray(record[key])) return record[key] as T[];
    }
  }
  return [];
}

function isBubbleObject(object: ArkObjectRecord): boolean {
  return (
    object.typeId === SYSTEM_TYPE_JOURNAL_ID &&
    object.propsJson?.entry_kind === "bubble" &&
    !object.deletedAt
  );
}

function objectToBubble(object: ArkObjectRecord): BubbleTimelineNode {
  const contentJson = readEntryTiptapDoc(object.contentJson) as JSONContent;
  const created = new Date(object.createdAt);
  return {
    id: object.id,
    createdAt: object.createdAt,
    updatedAt: object.updatedAt,
    date: Number.isNaN(created.getTime()) ? undefined : formatBubbleDateKey(created),
    time: Number.isNaN(created.getTime())
      ? ""
      : `${String(created.getHours()).padStart(2, "0")}:${String(created.getMinutes()).padStart(2, "0")}`,
    sortKey: Number.isNaN(created.getTime()) ? undefined : created.getTime(),
    text: bubblePlainText(contentJson),
    contentJson,
    tags: normalizeTags(object.propsJson?.tags),
    kind: normalizeBubbleKind(object.propsJson?.bubble_kind),
  };
}

function normalizeTags(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return [
    ...new Set(
      value
        .filter((tag): tag is string => typeof tag === "string")
        .map((tag) => tag.trim().toLocaleLowerCase("ru"))
        .filter(Boolean),
    ),
  ];
}

function linkId(childId: string, parentId: string): string {
  return `${childId}:${REPLY_LINK_TYPE}:${parentId}`;
}

async function migrationObjectId(namespace: string, sourceId: string): Promise<string> {
  const bytes = new TextEncoder().encode(`${namespace}\0${sourceId}`);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const hex = [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
  return `eden-bubble-${hex.slice(0, 32)}`;
}

function bubbleObject(
  bubble: BubbleTimelineNode,
  existing?: ArkObjectRecord | null,
): ArkObjectRecord {
  const occurrence = bubbleOccurrenceMillis(bubble);
  if (occurrence === null) throw new Error("Bubble occurrence timestamp is unresolved");
  const createdAt = existing?.createdAt ?? new Date(occurrence).toISOString();
  const updatedAt = bubble.updatedAt ?? new Date().toISOString();
  const contentJson = writeEntryTiptapDoc(
    (bubble.contentJson ?? plainTextToTiptapDoc(bubble.text)) as Parameters<
      typeof writeEntryTiptapDoc
    >[0],
  );
  return {
    id: bubble.id,
    typeId: SYSTEM_TYPE_JOURNAL_ID,
    title: bubble.text.slice(0, 80),
    contentJson,
    propsJson: {
      ...existing?.propsJson,
      entry_kind: "bubble",
      bubble_kind: normalizeBubbleKind(bubble.kind),
      tags: normalizeTags(bubble.tags),
    },
    createdAt,
    updatedAt,
    deletedAt: null,
  };
}

function sameMigratedBubble(object: ArkObjectRecord | null, expected: ArkObjectRecord): boolean {
  return Boolean(
    object &&
    isBubbleObject(object) &&
    object.id === expected.id &&
    object.createdAt === expected.createdAt &&
    object.propsJson.bubble_kind === expected.propsJson.bubble_kind &&
    JSON.stringify(normalizeTags(object.propsJson.tags)) ===
      JSON.stringify(expected.propsJson.tags) &&
    stableJson(object.contentJson) === stableJson(expected.contentJson),
  );
}

function stableJson(value: unknown): string | undefined {
  return JSON.stringify(value, (_key, nested) => {
    if (!nested || typeof nested !== "object" || Array.isArray(nested)) return nested;
    return Object.fromEntries(
      Object.entries(nested as Record<string, unknown>).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    );
  });
}

export function createBubbleApi(ark: ArkRequest) {
  async function listRaw(): Promise<{
    objects: ArkObjectRecord[];
    links: ArkObjectLinkRecord[];
  }> {
    const [objects, links] = await Promise.all([
      ark<unknown>("list_objects_by_type", {
        type_id: SYSTEM_TYPE_JOURNAL_ID,
      }).then(ensureList<ArkObjectRecord>),
      ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
    ]);
    return { objects: objects.filter(isBubbleObject), links };
  }

  async function listBubbles(): Promise<BubbleTimelineNode[]> {
    const { objects, links } = await listRaw();
    const replyLinks = links.filter((link) => link.linkType === REPLY_LINK_TYPE);
    const normalized = normalizeBubbleThreads(objects.map(objectToBubble), replyLinks);
    await Promise.all(
      normalized.invalidLinkIds.map((id) => ark("delete_object_link", { id }).catch(() => false)),
    );
    return normalized.bubbles;
  }

  async function createBubble(
    input: string,
    kind: BubbleKind = "plain",
    parentId?: string,
    contentJson?: BubbleTimelineNode["contentJson"],
  ): Promise<string> {
    const draft = parseBubbleDraft(input);
    if (!draft.text) throw new Error("Bubble text is empty");
    const now = new Date();
    const id = crypto.randomUUID();
    const node: BubbleTimelineNode = {
      id,
      createdAt: now.toISOString(),
      updatedAt: now.toISOString(),
      time: "",
      text: draft.text,
      contentJson: contentJson ?? plainTextToTiptapDoc(draft.text),
      tags: draft.tags,
      kind,
    };
    await ark("upsert_object", { object: bubbleObject(node) });
    if (parentId) {
      const timeline = await listBubbles();
      const parent = timeline.find((bubble) => bubble.id === parentId);
      if (!parent || parent.parentId) throw new Error("Replies can target roots only");
      await ark("upsert_object_link", {
        object_link: {
          id: linkId(id, parentId),
          sourceObjectId: id,
          targetObjectId: parentId,
          linkType: REPLY_LINK_TYPE,
          createdAt: now.toISOString(),
        },
      });
    }
    return id;
  }

  async function updateBubble(
    id: string,
    patch: { input?: string; kind?: BubbleKind },
  ): Promise<void> {
    const existing = await ark<ArkObjectRecord | null>("get_object", { id });
    if (!existing || !isBubbleObject(existing)) throw new Error("Bubble not found");
    const current = objectToBubble(existing);
    const draft = patch.input === undefined ? null : parseBubbleDraft(patch.input);
    if (draft && !draft.text) throw new Error("Bubble text is empty");
    const next: BubbleTimelineNode = {
      ...current,
      text: draft?.text ?? current.text,
      contentJson: draft ? plainTextToTiptapDoc(draft.text) : current.contentJson,
      tags: draft?.tags ?? current.tags,
      kind: patch.kind ?? current.kind,
      updatedAt: new Date().toISOString(),
    };
    await ark("upsert_object", { object: bubbleObject(next, existing) });
  }

  async function deleteBubble(id: string): Promise<void> {
    const links = await ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>);
    await Promise.all(
      links
        .filter(
          (link) =>
            link.linkType === REPLY_LINK_TYPE &&
            (link.sourceObjectId === id || link.targetObjectId === id),
        )
        .map((link) => ark("delete_object_link", { id: link.id })),
    );
    await ark("delete_object", { id });
  }

  async function migrateBubble(
    namespace: string,
    sourceId: string,
    source: BubbleTimelineNode,
  ): Promise<string> {
    if (bubbleOccurrenceMillis(source) === null) throw new Error("Unresolved legacy bubble date");
    const id = await migrationObjectId(namespace, sourceId);
    const expected = bubbleObject({
      ...source,
      id,
      updatedAt: source.updatedAt ?? source.createdAt,
    });
    await ark("upsert_object", { object: expected });
    const readBack = await ark<ArkObjectRecord | null>("get_object", { id });
    if (!sameMigratedBubble(readBack, expected))
      throw new Error("Bubble migration read-back mismatch");
    return id;
  }

  return {
    listBubbles,
    createBubble,
    updateBubble,
    deleteBubble,
    migrateBubble,
  };
}

export type BubbleApi = ReturnType<typeof createBubbleApi>;
