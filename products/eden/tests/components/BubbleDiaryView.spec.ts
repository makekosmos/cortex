import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import BubbleDiaryView from "../../src/components/bubbles/BubbleDiaryView.vue";
import {
  LOCAL_BUBBLES_STORAGE_KEY,
  encodeLocalBubblesStorage,
  formatBubbleDateKey,
  plainTextToTiptapDoc,
} from "../../src/components/bubbles/bubbleDiaryModel";
import { writeEntryTiptapDoc } from "../../src/editor-content/content";

type ArkObject = {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
};

const objects = new Map<string, ArkObject>();
const links = new Map<string, Record<string, unknown>>();
let request: ReturnType<typeof vi.fn>;
let subscribe: ReturnType<typeof vi.fn>;
const subscriptions = new Map<string, Set<(payload: unknown) => void>>();

function bubble(id: string, text: string, createdAt: string): ArkObject {
  return {
    id,
    typeId: "system-type-journal",
    title: text,
    contentJson: writeEntryTiptapDoc(plainTextToTiptapDoc(text)),
    propsJson: { entry_kind: "bubble", bubble_kind: "plain", tags: [] },
    createdAt,
    updatedAt: createdAt,
    deletedAt: null,
  };
}

function installArk() {
  subscriptions.clear();
  request = vi.fn(async (operation: string, params: Record<string, unknown> = {}) => {
    switch (operation) {
      case "list_objects_by_type":
        return [...objects.values()].filter((object) => object.typeId === params.type_id);
      case "list_object_links":
        return [...links.values()];
      case "get_object":
        return objects.get(String(params.id)) ?? null;
      case "upsert_object": {
        const object = structuredClone(params.object) as ArkObject;
        objects.set(object.id, object);
        emitArk("object_upserted", { id: object.id, type_id: object.typeId });
        return true;
      }
      case "delete_object": {
        const object = objects.get(String(params.id));
        if (object) object.deletedAt = new Date().toISOString();
        emitArk("object_deleted", { id: String(params.id) });
        return true;
      }
      case "upsert_object_link": {
        const link = structuredClone(params.object_link) as Record<string, unknown>;
        links.set(String(link.id), link);
        emitArk("entity_changed", { entity_type: "object_link", entity_id: link.id });
        return true;
      }
      case "delete_object_link":
        links.delete(String(params.id));
        emitArk("entity_changed", { entity_type: "object_link", entity_id: params.id });
        return true;
      default:
        throw new Error(`Unexpected ARK operation: ${operation}`);
    }
  });
  subscribe = vi.fn((event: string, handler: (payload: unknown) => void) => {
    const handlers = subscriptions.get(event) ?? new Set<(payload: unknown) => void>();
    handlers.add(handler);
    subscriptions.set(event, handlers);
    return () => handlers.delete(handler);
  });
  Object.defineProperty(window, "kepler", {
    configurable: true,
    value: { ark: { request, subscribe } },
  });
  window.api = {
    listAllEntries: vi.fn(async () => []),
    deleteEntry: vi.fn(async (entryId: string) => ({ ok: true, entryId })),
  } as unknown as Window["api"];
}

function emitArk(event: string, payload: unknown): void {
  for (const handler of subscriptions.get(event) ?? []) handler(payload);
}

describe("BubbleDiaryView", () => {
  beforeEach(() => {
    objects.clear();
    links.clear();
    localStorage.clear();
    installArk();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  test("loads from ARK and keeps ordinary journals out of the bubble timeline", async () => {
    objects.set("bubble-1", bubble("bubble-1", "Из ARK", new Date().toISOString()));
    objects.set("journal-1", {
      ...bubble("journal-1", "Обычный журнал", new Date().toISOString()),
      propsJson: {},
    });
    const screen = render(BubbleDiaryView);

    await expect.element(screen.getByTestId("bubble-node-bubble-1")).toHaveTextContent("Из ARK");
    await expect.element(screen.getByTestId("bubble-node-journal-1")).not.toBeInTheDocument();
  });

  test("renders saved task lists with the bubble Tiptap schema", async () => {
    const taskBubble = bubble("task-bubble", "Проверить", new Date().toISOString());
    taskBubble.contentJson = writeEntryTiptapDoc({
      type: "doc",
      content: [
        {
          type: "taskList",
          content: [
            {
              type: "taskItem",
              attrs: { checked: false },
              content: [{ type: "paragraph", content: [{ type: "text", text: "Проверить" }] }],
            },
          ],
        },
      ],
    });
    objects.set(taskBubble.id, taskBubble);
    const warn = vi.spyOn(console, "warn");

    const screen = render(BubbleDiaryView);

    await expect
      .element(screen.getByTestId("bubble-node-task-bubble"))
      .toHaveTextContent("Проверить");
    expect(warn.mock.calls.flat().join(" ")).not.toContain("Unknown node type: taskItem");
  });

  test("creates, edits, changes kind, and deletes through ARK without localStorage writes", async () => {
    const screen = render(BubbleDiaryView);
    await userEvent.type(screen.getByTestId("bubble-composer-input"), "Мысль #идея");
    await userEvent.click(screen.getByTestId("bubble-composer-submit"));
    await expect.element(screen.getByText("Мысль")).toBeInTheDocument();

    const created = [...objects.values()].find(
      (object) => object.propsJson.entry_kind === "bubble",
    );
    expect(created?.propsJson.tags).toEqual(["идея"]);
    expect(localStorage.getItem(LOCAL_BUBBLES_STORAGE_KEY)).toBeNull();

    await userEvent.click(screen.getByTestId(`bubble-time-${created?.id}`));
    await userEvent.clear(screen.getByTestId(`bubble-edit-input-${created?.id}`));
    await userEvent.type(screen.getByTestId(`bubble-edit-input-${created?.id}`), "Обновлено #тег");
    await userEvent.click(screen.getByText("Обновить"));
    expect(objects.get(created!.id)?.createdAt).toBe(created?.createdAt);
    expect(objects.get(created!.id)?.propsJson.tags).toEqual(["тег"]);

    const trigger = document.querySelector(`[data-testid="bubble-kind-${created?.id}"] button`);
    await userEvent.click(trigger as HTMLElement);
    await userEvent.click(screen.getByText("Идея"));
    expect(objects.get(created!.id)?.propsJson.bubble_kind).toBe("idea");

    await userEvent.click(screen.getByTestId(`bubble-time-${created?.id}`));
    await userEvent.click(screen.getByText("Удалить"));
    await userEvent.click(screen.getByText("Точно удалить"));
    expect(objects.get(created!.id)?.deletedAt).not.toBeNull();
  });

  test("migrates dated local records idempotently and preserves ambiguous time-only input", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([
        {
          id: "dated",
          date: "2026-07-01",
          time: "09:41",
          text: "Можно",
          tags: [],
          kind: "plain",
        },
        {
          id: "ambiguous",
          time: "09:41",
          text: "Нельзя угадывать",
          tags: [],
          kind: "plain",
        },
      ]),
    );

    render(BubbleDiaryView);
    await vi.waitFor(() =>
      expect([...objects.values()].some((object) => object.title === "Можно")).toBe(true),
    );
    expect([...objects.values()].some((object) => object.title === "Нельзя угадывать")).toBe(false);
    expect(
      JSON.parse(localStorage.getItem(LOCAL_BUBBLES_STORAGE_KEY) ?? "null").bubbles,
    ).toHaveLength(1);
  });

  test("renders one reply level and detaches replies when a root is deleted", async () => {
    objects.set("root", bubble("root", "Корень", "2026-07-10T08:00:00.000Z"));
    const screen = render(BubbleDiaryView);
    await expect.element(screen.getByTestId("bubble-node-root")).toBeInTheDocument();
    const replyAction = screen.getByTestId("bubble-reply-root");
    const replyActionElement = (await replyAction.element()) as HTMLElement;
    expect(getComputedStyle(replyActionElement).opacity).toBe("0");
    await userEvent.hover(screen.getByTestId("bubble-node-root"));
    await vi.waitFor(() => expect(getComputedStyle(replyActionElement).opacity).toBe("1"));
    await userEvent.click(screen.getByTestId("bubble-reply-root"));
    await userEvent.type(screen.getByTestId("bubble-reply-input-root"), "Ответ");
    const replyRenderStates: boolean[] = [];
    const replyObserver = new MutationObserver(() => {
      const replyNode = [
        ...document.querySelectorAll<HTMLElement>("[data-testid^='bubble-node-']"),
      ].find((element) => element.textContent?.includes("Ответ"));
      if (replyNode)
        replyRenderStates.push(replyNode.classList.contains("bubble-timeline-item--reply"));
    });
    replyObserver.observe(document.body, { childList: true, subtree: true, attributes: true });
    await userEvent.click(screen.getByTestId("bubble-reply-composer-root").getByText("Ответить"));

    const reply = [...objects.values()].find((object) => object.title === "Ответ");
    await expect
      .element(screen.getByTestId(`bubble-node-${reply?.id}`))
      .toHaveClass("bubble-timeline-item--reply");
    replyObserver.disconnect();
    expect(replyRenderStates).toContain(true);
    expect(replyRenderStates).not.toContain(false);
    await expect
      .element(screen.getByTestId("bubble-node-root"))
      .toHaveClass("bubble-timeline-item--continues-thread");
    expect(
      getComputedStyle(
        (await screen.getByTestId(`bubble-node-${reply?.id}`).element()) as HTMLElement,
      ).marginLeft,
    ).toBe("0px");
    const rootHeight = (
      (await screen.getByTestId("bubble-node-root").element()) as HTMLElement
    ).getBoundingClientRect().height;
    const replyHeight = (
      (await screen.getByTestId(`bubble-node-${reply?.id}`).element()) as HTMLElement
    ).getBoundingClientRect().height;
    expect(replyHeight).toBe(rootHeight);
    expect(
      (await screen.getByTestId("bubble-reply-root").element())
        .closest("[data-thread-id]")
        ?.querySelector(`[data-testid="bubble-node-${reply?.id}"]`),
    ).not.toBeNull();
    await expect.element(screen.getByTestId(`bubble-reply-${reply?.id}`)).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId("bubble-time-root"));
    await userEvent.click(screen.getByText("Удалить"));
    await userEvent.click(screen.getByText("Точно удалить"));
    await expect
      .element(screen.getByTestId(`bubble-node-${reply?.id}`))
      .not.toHaveClass("bubble-timeline-item--reply");
    expect(links.size).toBe(0);
  });

  test("calendar uses ARK occurrence dates", async () => {
    const today = new Date();
    objects.set("today", bubble("today", "Сегодня", today.toISOString()));
    const screen = render(BubbleDiaryView, { props: { calendarOpen: true } });
    await expect
      .element(screen.getByTestId(`diary-calendar-day-${formatBubbleDateKey(today)}`))
      .toHaveAttribute("data-entry-count", "1");
  });

  test("refreshes mounted labels at midnight, focus, and visibility without ARK writes", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 11, 31, 23, 59, 59, 900));
    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    objects.set(
      "midnight",
      bubble("midnight", "До полуночи", new Date(2026, 11, 31, 14, 5).toISOString()),
    );
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const screen = render(BubbleDiaryView);

    await vi.waitFor(() =>
      expect(request).toHaveBeenCalledWith("list_objects_by_type", expect.anything()),
    );
    await expect.element(screen.getByTestId("bubble-time-midnight")).toHaveTextContent("14:05");
    request.mockClear();

    await vi.advanceTimersByTimeAsync(150);
    await expect
      .element(screen.getByTestId("bubble-time-midnight"))
      .toHaveTextContent("Вчера, 14:05");

    vi.setSystemTime(new Date(2027, 0, 2, 12, 0));
    window.dispatchEvent(new Event("focus"));
    await expect
      .element(screen.getByTestId("bubble-time-midnight"))
      .toHaveTextContent("31 дек 2026, 14:05");

    vi.setSystemTime(new Date(2027, 0, 1, 12, 0));
    document.dispatchEvent(new Event("visibilitychange"));
    await expect
      .element(screen.getByTestId("bubble-time-midnight"))
      .toHaveTextContent("Вчера, 14:05");
    expect(request).not.toHaveBeenCalled();

    const clearCallsBeforeUnmount = clearTimeoutSpy.mock.calls.length;
    screen.unmount();
    expect(clearTimeoutSpy.mock.calls.length).toBeGreaterThan(clearCallsBeforeUnmount);
    visibility.mockRestore();
  });

  test("refreshes mounted objects and reply grouping from existing ARK events and reload", async () => {
    objects.set("root-event", bubble("root-event", "Корень события", "2026-07-10T08:00:00.000Z"));
    objects.set("reply-event", bubble("reply-event", "Ответ события", "2026-07-10T08:01:00.000Z"));
    const screen = render(BubbleDiaryView);

    await expect.element(screen.getByTestId("bubble-node-root-event")).toBeInTheDocument();
    await vi.waitFor(() => expect(subscribe).toHaveBeenCalledTimes(3));

    objects.set("live-event", bubble("live-event", "Живое обновление", "2026-07-10T08:02:00.000Z"));
    emitArk("object_upserted", {
      id: "live-event",
      type_id: "system-type-journal",
    });
    await expect.element(screen.getByTestId("bubble-node-live-event")).toBeInTheDocument();

    objects.get("live-event")!.deletedAt = new Date().toISOString();
    emitArk("object_deleted", { id: "live-event" });
    await expect.element(screen.getByTestId("bubble-node-live-event")).not.toBeInTheDocument();

    links.set("reply-event:reply_to:root-event", {
      id: "reply-event:reply_to:root-event",
      sourceObjectId: "reply-event",
      targetObjectId: "root-event",
      linkType: "reply_to",
    });
    emitArk("entity_changed", {
      entity_type: "object_link",
      entity_id: "reply-event:reply_to:root-event",
    });
    await expect
      .element(screen.getByTestId("bubble-node-reply-event"))
      .toHaveClass("bubble-timeline-item--reply");

    screen.unmount();
    expect([...subscriptions.values()].every((handlers) => handlers.size === 0)).toBe(true);
    const reloaded = render(BubbleDiaryView);
    await expect
      .element(reloaded.getByTestId("bubble-node-reply-event"))
      .toHaveClass("bubble-timeline-item--reply");
  });
});
