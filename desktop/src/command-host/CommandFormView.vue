<script setup lang="ts">
import { computed, shallowReactive, shallowRef } from "vue";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import CommandActionPanel from "./CommandActionPanel.vue";
import CommandDetailView from "./CommandDetailView.vue";
import { actionNodes } from "./model";
import { formModel } from "./model-form";

const props = defineProps<{
  root: CommandSnapshotNode;
  sessionId: string;
}>();

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
  if (typeof field.defaultValue === "boolean") {
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
  return typeof value === "string" ? value : "";
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
</script>

<template>
  <section class="command-form-view-shell">
    <form class="command-form-view" @submit.prevent="submitAction && executeAction(submitAction)">
      <div class="command-form-view__fields kosmos-scroll">
        <template v-for="field in form.fields" :key="field.id">
          <hr v-if="field.type === 'Form.Separator'" class="command-form-view__separator" />

          <p v-else-if="field.type === 'Form.Description'" class="command-form-view__description">
            {{ field.title }}
          </p>

          <label v-else-if="field.type === 'Form.Checkbox'" class="command-form-view__checkbox">
            <input
              :checked="checkboxValue(field.id)"
              class="command-form-view__checkbox-input"
              type="checkbox"
              @change="setBoolean(field.id, ($event.target as HTMLInputElement).checked)"
            />
            <span class="command-form-view__checkbox-title">{{ field.title }}</span>
          </label>

          <div v-else-if="field.type === 'Form.TagPicker'" class="command-form-view__field">
            <span class="command-form-view__label">{{ field.title }}</span>
            <div class="command-form-view__tag-picker">
              <button
                v-for="option in field.options"
                :key="`${field.id}:${option.value}`"
                class="command-form-view__tag"
                :class="{
                  'command-form-view__tag--selected': isTagSelected(field.id, option.value),
                }"
                type="button"
                :aria-pressed="isTagSelected(field.id, option.value)"
                @click="toggleTag(field.id, option.value)"
              >
                {{ option.title }}
              </button>
              <p v-if="field.options.length === 0" class="command-form-view__file-empty">
                Нет тегов
              </p>
            </div>
          </div>

          <label v-else class="command-form-view__field">
            <span class="command-form-view__label">{{ field.title }}</span>
            <textarea
              v-if="field.type === 'Form.TextArea'"
              class="command-form-view__textarea"
              :placeholder="field.placeholder ?? undefined"
              :value="inputValue(field.id)"
              @input="setString(field.id, ($event.target as HTMLTextAreaElement).value)"
            />
            <select
              v-else-if="field.type === 'Form.Dropdown'"
              class="command-form-view__input"
              :value="inputValue(field.id)"
              @change="setString(field.id, ($event.target as HTMLSelectElement).value)"
            >
              <template v-for="section in field.optionSections" :key="section.id">
                <template v-if="section.title">
                  <optgroup :label="section.title">
                    <option
                      v-for="option in section.options"
                      :key="`${section.id}:${option.value}`"
                      :value="option.value"
                    >
                      {{ option.title }}
                    </option>
                  </optgroup>
                </template>
                <template v-else>
                  <option
                    v-for="option in section.options"
                    :key="`${section.id}:${option.value}`"
                    :value="option.value"
                  >
                    {{ option.title }}
                  </option>
                </template>
              </template>
            </select>
            <div
              v-else-if="field.type === 'Form.FilePicker'"
              class="command-form-view__file-picker"
            >
              <button
                class="command-form-view__file-button"
                type="button"
                @click="chooseFiles(field.id)"
              >
                Выбрать
              </button>
              <div class="command-form-view__files">
                <p v-if="fileValues(field.id).length === 0" class="command-form-view__file-empty">
                  Файлы не выбраны
                </p>
                <button
                  v-for="path in fileValues(field.id)"
                  :key="path"
                  class="command-form-view__file"
                  type="button"
                  :title="path"
                  @click="removeFile(field.id, path)"
                >
                  {{ path }}
                </button>
              </div>
            </div>
            <input
              v-else
              class="command-form-view__input"
              :type="
                field.type === 'Form.PasswordField'
                  ? 'password'
                  : field.type === 'Form.DatePicker'
                    ? 'date'
                    : 'text'
              "
              :placeholder="field.placeholder ?? undefined"
              :value="inputValue(field.id)"
              @input="setString(field.id, ($event.target as HTMLInputElement).value)"
            />
          </label>
        </template>

        <p v-if="form.fields.length === 0" class="command-form-view__empty">Нет полей</p>
      </div>

      <footer class="command-form-view__footer">
        <CommandActionPanel :panel="form.actions" @execute="executeAction" />
        <p v-if="status" class="command-form-view__status">{{ status }}</p>
      </footer>
    </form>

    <CommandDetailView
      v-if="pushedDetail"
      :detail="pushedDetail"
      class="command-form-view-shell__detail"
    />
  </section>
</template>

<style scoped>
.command-form-view-shell {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  overflow: hidden;
}

.command-form-view {
  display: flex;
  min-height: 0;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.command-form-view__fields {
  display: grid;
  min-height: 0;
  gap: 14px;
  overflow: auto;
  padding: 18px;
}

.command-form-view__field {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.command-form-view__label,
.command-form-view__checkbox-title {
  color: var(--foreground);
  font-size: 13px;
  font-weight: 600;
}

.command-form-view__input,
.command-form-view__textarea,
.command-form-view__file-button,
.command-form-view__file,
.command-form-view__tag {
  width: 100%;
  border: 1px solid var(--input);
  border-radius: var(--radius-input);
  outline: none;
  background: color-mix(in srgb, var(--foreground) 5%, transparent);
  color: var(--foreground);
  padding: 0 10px;
  font-size: 13px;
}

.command-form-view__input,
.command-form-view__file-button {
  height: 34px;
}

.command-form-view__textarea {
  min-height: 90px;
  resize: vertical;
  padding-block: 8px;
}

.command-form-view__input:focus,
.command-form-view__textarea:focus,
.command-form-view__file-button:focus,
.command-form-view__file:focus,
.command-form-view__tag:focus {
  border-color: var(--accent);
}

.command-form-view__file-picker {
  display: grid;
  gap: 8px;
}

.command-form-view__file-button {
  width: max-content;
  min-width: 110px;
  cursor: default;
}

.command-form-view__files {
  display: grid;
  gap: 6px;
}

.command-form-view__file {
  min-width: 0;
  height: auto;
  padding-block: 8px;
  overflow: hidden;
  cursor: default;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-form-view__file-empty {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
}

.command-form-view__tag-picker {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.command-form-view__tag {
  width: auto;
  min-height: 28px;
  cursor: default;
}

.command-form-view__tag--selected {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
}

.command-form-view__checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
}

.command-form-view__checkbox-input {
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}

.command-form-view__empty,
.command-form-view__status,
.command-form-view__description {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 12px;
}

.command-form-view__description {
  line-height: 1.5;
}

.command-form-view__separator {
  width: 100%;
  height: 1px;
  border: 0;
  background: var(--border);
}

.command-form-view__footer {
  display: flex;
  min-height: 44px;
  align-items: center;
  gap: 10px;
  border-top: 1px solid var(--border);
  padding: 8px 10px;
}

.command-form-view-shell__detail {
  flex: 0 0 min(380px, 42%);
}
</style>
