import { computed, shallowReactive, shallowRef } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { isBoolean, isString } from "../shared/runtimeGuards";
import { actionNodes } from "./model";
import { formModel } from "./model-form";

export function useCommandFormView(props: { root: CommandSnapshotNode; sessionId: string }) {
  const values = shallowReactive<Record<string, string | boolean | string[]>>({});
  const status = shallowRef<string | null>(null);
  const pushedDetail = shallowRef<CommandSnapshotNode | null>(null);

  const form = computed(() => formModel(props.root));
  const submitAction = computed(
    () =>
      actionNodes(form.value.actions).find((action) => action.type === "Action.SubmitForm") ?? null,
  );

  for (const field of form.value.fields) {
    if (field.type === "Form.Description" || field.type === "Form.Separator") continue;
    if (isBoolean(field.defaultValue)) {
      values[field.id] = field.defaultValue;
    } else if (Array.isArray(field.defaultValue)) {
      values[field.id] = [...field.defaultValue];
    } else {
      values[field.id] = field.defaultValue ?? "";
    }
  }

  async function executeAction(action: CommandSnapshotNode): Promise<void> {
    if (action.type === "Action.Push") {
      pushedDetail.value =
        action.children.find((child) => child.type === "Detail") ?? action.children[0] ?? null;
      status.value = "Открыто";
      return;
    }

    if (
      action.type === "Action" ||
      action.type === "Action.CopyToClipboard" ||
      action.type === "Action.Paste" ||
      action.type === "Action.Pop" ||
      action.type === "Action.PopToRoot" ||
      action.type === "Action.OpenInBrowser" ||
      action.type === "Action.Open" ||
      action.type === "Action.ShowInFinder" ||
      action.type === "Action.Trash" ||
      action.type === "Action.LaunchCommand"
    ) {
      const result = await window.kepler.command.action(props.sessionId, {
        type: action.type,
        props: action.props,
      });
      status.value = result.ok ? actionSuccessMessage(action.type) : "Действие не выполнено";
      return;
    }
    if (action.type !== "Action.SubmitForm") return;
    const result = await window.kepler.command.action(props.sessionId, {
      type: action.type,
      props: action.props,
      payload: { values: { ...values } },
    });
    status.value = result.ok ? "Отправлено" : "Не удалось отправить";
  }

  function actionSuccessMessage(type: string): string {
    if (type === "Action") return "Готово";
    if (type === "Action.CopyToClipboard") return "Скопировано";
    if (type === "Action.Paste") return "Вставлено";
    if (type === "Action.Pop") return "Назад";
    if (type === "Action.PopToRoot") return "К началу";
    if (type === "Action.OpenInBrowser" || type === "Action.Open") return "Открыто";
    if (type === "Action.ShowInFinder") return "Показано";
    if (type === "Action.Trash") return "Удалено";
    if (type === "Action.LaunchCommand") return "Запущено";
    return "Готово";
  }

  function inputValue(id: string): string {
    const value = values[id];
    return isString(value) ? value : "";
  }

  function fileValues(id: string): string[] {
    const value = values[id];
    return Array.isArray(value) ? value : [];
  }

  function isTagSelected(fieldId: string, value: string): boolean {
    return fileValues(fieldId).includes(value);
  }

  function toggleTag(fieldId: string, value: string): void {
    const current = fileValues(fieldId);
    setFiles(
      fieldId,
      current.includes(value) ? current.filter((item) => item !== value) : [...current, value],
    );
  }

  function setString(id: string, value: string): void {
    values[id] = value;
    status.value = null;
    void notifyFieldChange(id, value);
  }

  function setFiles(id: string, paths: string[]): void {
    values[id] = paths;
    status.value = null;
    void notifyFieldChange(id, paths);
  }

  async function chooseFiles(fieldId: string): Promise<void> {
    const field = form.value.fields.find((item) => item.id === fieldId);
    if (!field) return;
    const result = await window.kepler.command.pickFiles(props.sessionId, {
      allowMultipleSelection: field.allowMultipleSelection,
      canChooseDirectories: field.canChooseDirectories,
      canChooseFiles: field.canChooseFiles,
      showHiddenFiles: field.showHiddenFiles,
    });
    if (!result.ok) {
      status.value = "Не удалось выбрать файл";
      return;
    }
    if (result.paths.length > 0) setFiles(fieldId, result.paths);
  }

  function removeFile(fieldId: string, path: string): void {
    setFiles(
      fieldId,
      fileValues(fieldId).filter((item) => item !== path),
    );
  }

  function checkboxValue(id: string): boolean {
    return values[id] === true;
  }

  function setBoolean(id: string, value: boolean): void {
    values[id] = value;
    status.value = null;
    void notifyFieldChange(id, value);
  }

  async function notifyFieldChange(id: string, value: string | boolean | string[]): Promise<void> {
    const field = form.value.fields.find((item) => item.id === id);
    if (!field?.callbackId) return;
    const result = await window.kepler.command.action(props.sessionId, {
      type: field.type,
      props: { __callbackId: field.callbackId },
      payload: { value },
    });
    status.value = result.ok ? "Обновлено" : "Изменение не применено";
  }

  return {
    values,
    status,
    pushedDetail,
    form,
    submitAction,
    executeAction,
    actionSuccessMessage,
    inputValue,
    fileValues,
    isTagSelected,
    toggleTag,
    setString,
    setFiles,
    chooseFiles,
    removeFile,
    checkboxValue,
    setBoolean,
    notifyFieldChange,
  };
}
