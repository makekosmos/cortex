import { describe, expect, test } from "bun:test";
import {
  Action,
  ActionPanel,
  Detail,
  Form,
  Grid,
  Keyboard,
  List,
  MenuBarExtra,
} from "../../packages/raycast-api/src/index";
import { normalizeRaycastNode } from "../../platform/desktop/electron/raycast/view-model";
import { parseRaycastMarkdown } from "../../platform/desktop/src/raycast-host/markdown";
import {
  actionNodes,
  actionSections,
  actionShortcut,
  actionSubmenuActions,
  detailActions,
  detailMetadataItems,
  detailMarkdown,
  formModel,
  gridDropdown,
  gridEmptyActions,
  gridEmptyMessage,
  gridFiltering,
  gridItems,
  gridIsLoading,
  gridPlaceholder,
  gridSearchCallbackNode,
  gridSearchText,
  gridSections,
  gridSelectedItemId,
  gridSelectionCallbackNode,
  listEmptyActions,
  listEmptyMessage,
  listDropdown,
  listItems,
  listFiltering,
  listIsLoading,
  listPlaceholder,
  listSearchCallbackNode,
  listSearchText,
  listSections,
  listSelectedItemId,
  listSelectionCallbackNode,
  matchesActionShortcut,
  menuBarSections,
  menuBarTitle,
} from "../../platform/desktop/src/raycast-host/model";

describe("Raycast view model", () => {
  test("normalizes List.Item detail/actions props into host-renderable children", () => {
    const root = List({
      searchBarPlaceholder: "Search notes",
      searchBarAccessory: List.Dropdown({
        defaultValue: "all",
        children: [
          List.Dropdown.Item({ value: "all", title: "Все" }),
          List.Dropdown.Section({
            title: "Тип",
            children: [
              List.Dropdown.Item({ value: "notes", title: "Заметки" }),
              List.Dropdown.Item({ value: "tasks", title: "Задачи" }),
            ],
          }),
        ],
      }),
      children: [
        List.Section({
          title: "Заметки",
          children: [
            List.Item({
              id: "hello",
              title: "Hello",
              subtitle: "First",
              icon: { source: "data:image/png;base64,abc" },
              keywords: ["note"],
              accessories: [{ text: "2 задачи" }, { tag: "Сегодня" }],
              detail: List.Item.Detail({
                markdown: "# Hello",
                metadata: List.Item.Detail.Metadata({
                  children: [List.Item.Detail.Metadata.Label({ title: "Тип", text: "Заметка" })],
                }),
              }),
              actions: ActionPanel({
                children: [
                  Action.CopyToClipboard({ content: "Hello" }),
                  Action.Paste({ content: "Hello" }),
                  Action.Push({ title: "Открыть", target: Detail({ markdown: "# Pushed" }) }),
                  Action.OpenInBrowser({ url: "https://example.com" }),
                  Action.Open({ target: "C:/Temp/example.txt" }),
                  Action.ShowInFinder({ path: "C:/Temp/example.txt" }),
                  Action.Trash({ paths: ["C:/Temp/delete.txt"] }),
                  Action({ title: "Произвольное", onAction: () => {} }),
                  Action.LaunchCommand({
                    title: "Запустить copy",
                    name: "copy",
                    arguments: { from: "list" },
                  }),
                ],
              }),
            }),
          ],
        }),
        List.EmptyView({
          title: "Пусто",
          actions: ActionPanel({
            children: [
              Action.CopyToClipboard({ title: "Скопировать пустое", content: "empty-list" }),
            ],
          }),
        }),
        List.Item({
          id: "loose",
          title: "Loose",
          actions: ActionPanel({
            children: [Action.CopyToClipboard({ content: "Loose" })],
          }),
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root);
    expect(snapshot?.type).toBe("List");
    expect(snapshot ? listIsLoading(snapshot) : false).toBe(false);
    expect(snapshot ? listPlaceholder(snapshot) : "").toBe("Search notes");
    expect(snapshot ? listEmptyMessage(snapshot) : "").toBe("Пусто");
    expect(
      actionNodes(snapshot ? listEmptyActions(snapshot) : null).map((node) => node.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(snapshot ? listDropdown(snapshot)?.defaultValue : "").toBe("all");
    expect(
      snapshot ? listDropdown(snapshot)?.sections.map((section) => section.title) : [],
    ).toEqual([null, "Тип"]);
    expect(snapshot ? listDropdown(snapshot)?.options.map((option) => option.value) : []).toEqual([
      "all",
      "notes",
      "tasks",
    ]);

    const items = snapshot ? listItems(snapshot) : [];
    expect(items).toHaveLength(2);
    expect(items[0]?.title).toBe("Hello");
    expect(items[0]?.icon).toBe("data:image/png;base64,abc");
    expect(items[0]?.keywords).toEqual(["note"]);
    expect(items[0]?.accessories).toEqual(["2 задачи", "Сегодня"]);
    expect(items[1]?.title).toBe("Loose");
    const sections = snapshot ? listSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Заметки", null]);
    expect(sections.map((section) => section.items.map((item) => item.id))).toEqual([
      ["hello"],
      ["loose"],
    ]);
    expect(detailMarkdown(items[0]?.detail ?? null)).toBe("# Hello");
    expect(detailMetadataItems(items[0]?.detail ?? null)).toEqual([
      { id: "metadata:0", type: "label", title: "Тип", text: "Заметка", href: null },
    ]);
    const actions = actionNodes(items[0]?.actions ?? null);
    expect(actions.map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
      "Action.Paste",
      "Action.Push",
      "Action.OpenInBrowser",
      "Action.Open",
      "Action.ShowInFinder",
      "Action.Trash",
      "Action",
      "Action.LaunchCommand",
    ]);
    expect(actions[0]?.props.__callbackId).toBeUndefined();
    expect(actions.find((node) => node.type === "Action.Push")?.children[0]?.props.markdown).toBe(
      "# Pushed",
    );
    expect(actions.find((node) => node.type === "Action.OpenInBrowser")?.props.url).toBe(
      "https://example.com",
    );
    expect(actions.find((node) => node.type === "Action.Open")?.props.target).toBe(
      "C:/Temp/example.txt",
    );
    expect(actions.find((node) => node.type === "Action.ShowInFinder")?.props.path).toBe(
      "C:/Temp/example.txt",
    );
    expect(actions.find((node) => node.type === "Action.Trash")?.props.paths).toEqual([
      "C:/Temp/delete.txt",
    ]);
    expect(actions.find((node) => node.type === "Action.LaunchCommand")?.props.name).toBe("copy");
    expect(actions.find((node) => node.type === "Action.LaunchCommand")?.props.arguments).toEqual({
      from: "list",
    });
  });

  test("normalizes List controlled props and callbacks into host props", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    let searched = "";
    let selected: string | null = null;
    const root = List({
      searchText: "черновик",
      selectedItemId: "two",
      filtering: false,
      onSearchTextChange: (text) => {
        searched = text;
      },
      onSelectionChange: (id) => {
        selected = id;
      },
      children: [
        List.Item({ id: "one", title: "Первый" }),
        List.Item({ id: "two", title: "Второй" }),
      ],
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("List");
    expect(snapshot ? listSearchText(snapshot) : "").toBe("черновик");
    expect(snapshot ? listSelectedItemId(snapshot) : null).toBe("two");
    expect(snapshot ? listFiltering(snapshot) : true).toBe(false);
    expect(snapshot ? listSearchCallbackNode(snapshot)?.props.__onSearchTextChangeId : null).toBe(
      "callback:0",
    );
    expect(snapshot ? listSelectionCallbackNode(snapshot)?.props.__onSelectionChangeId : null).toBe(
      "callback:1",
    );

    callbacks.get("callback:0")?.({ text: "готово" });
    callbacks.get("callback:1")?.({ id: null });
    expect(searched).toBe("готово");
    expect(selected).toBe(null);
  });

  test("parses Raycast Detail markdown into safe render blocks", () => {
    expect(
      parseRaycastMarkdown("# Заголовок\n\nТекст\n\n- Один\n- Два\n\n```ts\nconst ok = true;\n```"),
    ).toEqual([
      { type: "heading", level: 1, text: "Заголовок" },
      { type: "paragraph", text: "Текст" },
      { type: "list", items: ["Один", "Два"] },
      { type: "code", text: "const ok = true;" },
    ]);
  });

  test("normalizes Detail.Metadata into host metadata rows", () => {
    const root = Detail({
      markdown: "# Проект",
      metadata: Detail.Metadata({
        children: [
          Detail.Metadata.Label({ title: "Статус", text: "Активен" }),
          Detail.Metadata.Link({
            title: "Ссылка",
            text: "Документация",
            target: "https://example.com/docs",
          }),
          Detail.Metadata.Separator({}),
          Detail.Metadata.TagList({
            title: "Теги",
            children: [
              Detail.Metadata.TagList.Item({ text: "raycast" }),
              Detail.Metadata.TagList.Item({ text: "kosmos" }),
            ],
          }),
        ],
      }),
    });

    const snapshot = normalizeRaycastNode(root);
    expect(snapshot?.type).toBe("Detail");
    expect(snapshot ? detailMarkdown(snapshot) : "").toBe("# Проект");
    expect(snapshot ? detailMetadataItems(snapshot) : []).toEqual([
      { id: "metadata:0", type: "label", title: "Статус", text: "Активен", href: null },
      {
        id: "metadata:1",
        type: "link",
        title: "Ссылка",
        text: "Документация",
        href: "https://example.com/docs",
      },
      { id: "metadata:2", type: "separator" },
      { id: "metadata:3", type: "tags", title: "Теги", tags: ["raycast", "kosmos"] },
    ]);
  });

  test("normalizes root Detail actions without leaking pushed detail markdown", () => {
    const root = Detail({
      markdown: "# Проект",
      actions: ActionPanel({
        children: [
          Action.CopyToClipboard({ title: "Скопировать", content: "Проект" }),
          Action.Push({ title: "Открыть", target: Detail({ markdown: "# Вложенный экран" }) }),
        ],
      }),
    });

    const snapshot = normalizeRaycastNode(root);
    expect(snapshot?.children.map((node) => node.type)).toEqual(["ActionPanel"]);
    expect(snapshot ? detailMarkdown(snapshot) : "").toBe("# Проект");
    expect(actionNodes(snapshot ? detailActions(snapshot) : null).map((node) => node.type)).toEqual(
      ["Action.CopyToClipboard", "Action.Push"],
    );
  });

  test("groups ActionPanel.Section actions for host rendering", () => {
    const root = ActionPanel({
      children: [
        Action({ title: "Корневое" }),
        Action.Pop({ shortcut: { modifiers: ["cmd"], key: "arrowLeft" } }),
        Action.PopToRoot({ title: "Домой" }),
        ActionPanel.Submenu({
          title: "Ещё",
          children: [Action.CopyToClipboard({ title: "Скопировать", content: "Проект" })],
        }),
        ActionPanel.Section({
          title: "Файлы",
          children: [
            Action.Open({ title: "Открыть", target: "C:\\Temp\\note.txt" }),
            Action.ShowInFinder({ title: "Показать", path: "C:\\Temp\\note.txt" }),
          ],
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root);
    const sections = actionSections(snapshot);

    expect(sections.map((section) => section.title)).toEqual([null, "Файлы"]);
    expect(sections.map((section) => section.actions.map((action) => action.type))).toEqual([
      ["Action", "Action.Pop", "Action.PopToRoot", "ActionPanel.Submenu"],
      ["Action.Open", "Action.ShowInFinder"],
    ]);
    expect(actionNodes(snapshot).map((action) => action.type)).toEqual([
      "Action",
      "Action.Pop",
      "Action.PopToRoot",
      "Action.CopyToClipboard",
      "Action.Open",
      "Action.ShowInFinder",
    ]);
    expect(
      actionSubmenuActions(sections[0]?.actions[3] ?? snapshot).map((action) => action.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(actionShortcut(actionNodes(snapshot)[1])?.label).toBe("Cmd arrowLeft");
  });

  test("normalizes and matches Action shortcuts by KeyboardEvent code", () => {
    const root = ActionPanel({
      children: [
        Action.CopyToClipboard({
          title: "Скопировать",
          content: "Проект",
          shortcut: Keyboard.Shortcut.Common.Copy,
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root);
    const shortcut = actionShortcut(actionNodes(snapshot)[0]);

    expect(shortcut?.label).toBe("Cmd C");
    expect(shortcut?.code).toBe("KeyC");
    expect(
      matchesActionShortcut(shortcut, {
        altKey: false,
        code: "KeyC",
        ctrlKey: false,
        key: "с",
        metaKey: true,
        shiftKey: false,
      }),
    ).toBe(true);
    expect(
      matchesActionShortcut(shortcut, {
        altKey: false,
        code: "KeyC",
        ctrlKey: true,
        key: "c",
        metaKey: true,
        shiftKey: true,
      }),
    ).toBe(false);
  });

  test("normalizes Form fields and submit callback into host model", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    let changedDate: Date | null | undefined;
    const root = Form({
      children: [
        Form.TextField({ id: "title", title: "Заголовок", defaultValue: "Черновик" }),
        Form.PasswordField({ id: "secret", title: "Секрет" }),
        Form.Description({ text: "Описание формы" }),
        Form.Separator({}),
        Form.TextArea({ id: "body", title: "Текст", placeholder: "Введите текст" }),
        Form.Checkbox({ id: "flagged", title: "Важное", defaultValue: true }),
        Form.Dropdown({
          id: "kind",
          title: "Тип",
          defaultValue: "note",
          onChange: (value) => {
            callbacks.set("kind", () => value);
          },
          children: [
            Form.Dropdown.Item({ value: "inbox", title: "Входящее" }),
            Form.Dropdown.Section({
              title: "Тип",
              children: [
                Form.Dropdown.Item({ value: "note", title: "Заметка" }),
                Form.Dropdown.Item({ value: "task", title: "Задача" }),
              ],
            }),
          ],
        }),
        Form.TagPicker({
          id: "tags",
          title: "Теги",
          defaultValue: ["kosmos"],
          onChange: (value) => {
            callbacks.set("tags", () => value);
          },
          children: [
            Form.TagPicker.Item({ value: "kosmos", title: "Kosmos" }),
            Form.TagPicker.Item({ value: "raycast", title: "Raycast" }),
          ],
        }),
        Form.FilePicker({
          id: "attachments",
          title: "Вложения",
          defaultValue: ["C:/Temp/example.txt"],
          allowMultipleSelection: true,
          canChooseDirectories: true,
        }),
        Form.DatePicker({
          id: "due",
          title: "Срок",
          defaultValue: new Date("2026-06-04T12:30:00.000Z"),
          onChange: (value) => {
            changedDate = value;
          },
        }),
      ],
      actions: ActionPanel({
        children: [
          Action.SubmitForm({
            title: "Создать",
            onSubmit: (values) => {
              callbacks.set("submitted", () => values);
            },
          }),
          Action.CopyToClipboard({ title: "Скопировать", content: "Форма" }),
          Action.OpenInBrowser({ title: "Открыть сайт", url: "https://example.com" }),
          Action.Open({ title: "Открыть файл", target: "C:/Temp/example.txt" }),
          Action.Push({ title: "Детали", target: Detail({ markdown: "# Форма" }) }),
        ],
      }),
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });
    expect(snapshot?.type).toBe("Form");

    const model = snapshot ? formModel(snapshot) : { fields: [], actions: null };
    expect(model.fields.map((field) => [field.id, field.type, field.defaultValue])).toEqual([
      ["title", "Form.TextField", "Черновик"],
      ["secret", "Form.PasswordField", null],
      ["2:Form.Description", "Form.Description", null],
      ["3:Form.Separator", "Form.Separator", null],
      ["body", "Form.TextArea", null],
      ["flagged", "Form.Checkbox", true],
      ["kind", "Form.Dropdown", "note"],
      ["tags", "Form.TagPicker", ["kosmos"]],
      ["attachments", "Form.FilePicker", ["C:/Temp/example.txt"]],
      ["due", "Form.DatePicker", "2026-06-04"],
    ]);
    expect(model.fields.find((field) => field.id === "2:Form.Description")?.title).toBe(
      "Описание формы",
    );
    const kindField = model.fields.find((field) => field.id === "kind");
    expect(kindField?.callbackId).toBeString();
    expect(kindField?.options).toEqual([
      { value: "inbox", title: "Входящее" },
      { value: "note", title: "Заметка" },
      { value: "task", title: "Задача" },
    ]);
    expect(kindField?.optionSections).toEqual([
      {
        id: "form-dropdown-section:root:0",
        title: null,
        options: [{ value: "inbox", title: "Входящее" }],
      },
      {
        id: "form-dropdown-section:1",
        title: "Тип",
        options: [
          { value: "note", title: "Заметка" },
          { value: "task", title: "Задача" },
        ],
      },
    ]);
    const tagsField = model.fields.find((field) => field.id === "tags");
    expect(tagsField?.callbackId).toBeString();
    expect(tagsField?.options).toEqual([
      { value: "kosmos", title: "Kosmos" },
      { value: "raycast", title: "Raycast" },
    ]);
    const attachmentsField = model.fields.find((field) => field.id === "attachments");
    expect(attachmentsField?.allowMultipleSelection).toBe(true);
    expect(attachmentsField?.canChooseDirectories).toBe(true);
    expect(attachmentsField?.canChooseFiles).toBe(true);
    const dueField = model.fields.find((field) => field.id === "due");
    expect(dueField?.callbackId).toBeString();

    const submitAction = actionNodes(model.actions)[0];
    expect(submitAction?.type).toBe("Action.SubmitForm");
    expect(submitAction?.props.__callbackId).toBeString();
    expect(actionNodes(model.actions).map((action) => action.type)).toEqual([
      "Action.SubmitForm",
      "Action.CopyToClipboard",
      "Action.OpenInBrowser",
      "Action.Open",
      "Action.Push",
    ]);
    expect(
      actionNodes(model.actions).find((action) => action.type === "Action.Push")?.children[0]?.props
        .markdown,
    ).toBe("# Форма");
    callbacks.get(kindField?.callbackId ?? "")?.({ value: "task" });
    callbacks.get(tagsField?.callbackId ?? "")?.({ value: ["kosmos", "raycast"] });
    callbacks.get(dueField?.callbackId ?? "")?.({ value: "2026-06-05" });
    callbacks.get(String(submitAction?.props.__callbackId))?.({ values: { title: "Готово" } });
    expect(callbacks.get("kind")?.()).toBe("task");
    expect(callbacks.get("tags")?.()).toEqual(["kosmos", "raycast"]);
    expect(changedDate?.toISOString()).toBe("2026-06-05T00:00:00.000Z");
    expect(callbacks.get("submitted")?.()).toEqual({ title: "Готово" });
  });

  test("normalizes Grid sections, items, images, and actions into host model", () => {
    const root = Grid({
      searchBarPlaceholder: "Поиск игр",
      searchBarAccessory: Grid.Dropdown({
        defaultValue: "all",
        children: [
          Grid.Dropdown.Item({ value: "all", title: "Все" }),
          Grid.Dropdown.Section({
            title: "Статус",
            children: [
              Grid.Dropdown.Item({ value: "installed", title: "Установлено" }),
              Grid.Dropdown.Item({ value: "backlog", title: "Бэклог" }),
            ],
          }),
        ],
      }),
      children: [
        Grid.Section({
          title: "Игры",
          children: [
            Grid.Item({
              id: "one",
              title: "Первая",
              subtitle: "Steam",
              content: { source: "data:image/png;base64,abc" },
              keywords: ["game"],
              actions: ActionPanel({
                children: [Action.CopyToClipboard({ content: "one" })],
              }),
            }),
          ],
        }),
        Grid.EmptyView({
          title: "Нет элементов",
          actions: ActionPanel({
            children: [
              Action.CopyToClipboard({ title: "Скопировать пустую сетку", content: "empty-grid" }),
            ],
          }),
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root);
    expect(snapshot?.type).toBe("Grid");
    expect(snapshot ? gridIsLoading(snapshot) : false).toBe(false);
    expect(snapshot ? gridPlaceholder(snapshot) : "").toBe("Поиск игр");
    expect(snapshot ? gridEmptyMessage(snapshot) : "").toBe("Нет элементов");
    expect(
      actionNodes(snapshot ? gridEmptyActions(snapshot) : null).map((node) => node.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(snapshot ? gridDropdown(snapshot)?.defaultValue : "").toBe("all");
    expect(
      snapshot ? gridDropdown(snapshot)?.sections.map((section) => section.title) : [],
    ).toEqual([null, "Статус"]);
    expect(snapshot ? gridDropdown(snapshot)?.options.map((option) => option.value) : []).toEqual([
      "all",
      "installed",
      "backlog",
    ]);

    const sections = snapshot ? gridSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Игры"]);
    const items = snapshot ? gridItems(snapshot) : [];
    expect(items).toHaveLength(1);
    expect(items[0]?.image).toBe("data:image/png;base64,abc");
    expect(actionNodes(items[0]?.actions ?? null).map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
    ]);
  });

  test("normalizes Grid controlled props and callbacks into host props", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    let searched = "";
    let selected: string | null = null;
    const root = Grid({
      searchText: "игра",
      selectedItemId: "outer-wilds",
      filtering: false,
      onSearchTextChange: (text) => {
        searched = text;
      },
      onSelectionChange: (id) => {
        selected = id;
      },
      children: [
        Grid.Item({ id: "disco", title: "Disco Elysium" }),
        Grid.Item({ id: "outer-wilds", title: "Outer Wilds" }),
      ],
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("Grid");
    expect(snapshot ? gridSearchText(snapshot) : "").toBe("игра");
    expect(snapshot ? gridSelectedItemId(snapshot) : null).toBe("outer-wilds");
    expect(snapshot ? gridFiltering(snapshot) : true).toBe(false);
    expect(snapshot ? gridSearchCallbackNode(snapshot)?.props.__onSearchTextChangeId : null).toBe(
      "callback:0",
    );
    expect(snapshot ? gridSelectionCallbackNode(snapshot)?.props.__onSelectionChangeId : null).toBe(
      "callback:1",
    );

    callbacks.get("callback:0")?.({ text: "космос" });
    callbacks.get("callback:1")?.({ id: "disco" });
    expect(searched).toBe("космос");
    expect(selected).toBe("disco");
  });

  test("normalizes MenuBarExtra sections, submenus, and callbacks into host model", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    const root = MenuBarExtra({
      title: "Статус",
      children: [
        MenuBarExtra.Section({
          title: "Синхронизация",
          children: [
            MenuBarExtra.Item({
              id: "sync",
              title: "Синхронизировать",
              subtitle: "Сейчас",
              onAction: () => callbacks.set("ran", () => true),
            }),
            MenuBarExtra.Submenu({
              id: "more",
              title: "Ещё",
              children: [MenuBarExtra.Item({ id: "logs", title: "Логи" })],
            }),
          ],
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("MenuBarExtra");
    expect(snapshot ? menuBarTitle(snapshot) : "").toBe("Статус");
    const sections = snapshot ? menuBarSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Синхронизация"]);
    expect(sections[0]?.items.map((item) => [item.id, item.title, item.subtitle])).toEqual([
      ["sync", "Синхронизировать", "Сейчас"],
      ["more", "Ещё", null],
    ]);
    expect(sections[0]?.items[1]?.children.map((item) => item.title)).toEqual(["Логи"]);
    expect(sections[0]?.items[0]?.node.props.__callbackId).toBe("callback:0");
    callbacks.get("callback:0")?.();
    expect(callbacks.get("ran")?.()).toBe(true);
  });

  test("extracts loading state from List and Grid roots", () => {
    const listSnapshot = normalizeRaycastNode(List({ isLoading: true }));
    const gridSnapshot = normalizeRaycastNode(Grid({ isLoading: true }));

    expect(listSnapshot ? listIsLoading(listSnapshot) : false).toBe(true);
    expect(gridSnapshot ? gridIsLoading(gridSnapshot) : false).toBe(true);
  });
});
