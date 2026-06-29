import { describe, expect, test } from "bun:test";
import { Action, ActionPanel, Detail, Form } from "../../packages/raycast-api/src/index";
import { normalizeCommandNode } from "../../platform/desktop/electron/command-host/view-model";
import { actionNodes } from "../../platform/desktop/src/command-host/model";
import { formModel } from "../../platform/desktop/src/command-host/model-form";

describe("Command form view model", () => {
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

    const snapshot = normalizeCommandNode(root, {
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
});
