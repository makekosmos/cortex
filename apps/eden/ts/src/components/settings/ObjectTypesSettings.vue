<template>
  <div class="object-types-layout">
    <!-- Left: type list -->
    <div class="object-types-list">
      <div class="object-types-list-head">
        <div class="object-types-filter">
          <input
            class="object-types-search"
            type="text"
            placeholder="Поиск"
            v-model="filterQuery"
          />
        </div>
        <button
          class="settings-btn-secondary object-types-new-btn"
          type="button"
          @click="openTypeEditor()"
        >
          Новый
        </button>
      </div>
      <div class="object-types-items">
        <div class="object-types-section-name">Системные типы</div>
        <div v-for="bt in builtInTypes" :key="bt.name" class="object-types-item builtin">
          <img
            class="object-types-item-icon"
            :src="`/anytype/icon/type/default/${bt.icon}.svg`"
            alt=""
            width="18"
            height="18"
            draggable="false"
          />
          <span class="object-types-item-name">{{ bt.name }}</span>
        </div>
        <div v-if="filteredTypes.length > 0" class="object-types-section-name">Мои типы</div>
        <button
          v-for="noteType in filteredTypes"
          :key="noteType.id"
          class="object-types-item"
          :class="{ active: selectedTypeId === noteType.id }"
          type="button"
          @click="openTypeEditor(noteType)"
        >
          <img
            class="object-types-item-icon"
            :src="`/anytype/icon/type/default/${noteType.icon || 'document'}.svg`"
            alt=""
            width="18"
            height="18"
            draggable="false"
          />
          <span class="object-types-item-name">{{ noteType.name }}</span>
        </button>
      </div>
    </div>

    <!-- Right: type editor -->
    <div class="object-types-editor">
      <div v-if="typeDraft" class="type-editor">
        <div class="type-editor-head">
          <button class="settings-btn-secondary" type="button" @click="closeTypeEditor">
            Отмена
          </button>
          <button class="settings-btn-primary" type="button" @click="submitTypeEditor">
            Сохранить
          </button>
        </div>
        <div class="type-editor-body">
          <!-- Name -->
          <div class="type-editor-section">
            <div class="type-editor-label">Название типа</div>
            <div class="type-editor-name-row">
              <div class="type-editor-icon-btn-wrap" style="position: relative">
                <button
                  class="type-editor-icon-btn"
                  type="button"
                  title="Выбрать иконку"
                  @click="iconPickerOpen = !iconPickerOpen"
                >
                  <img
                    :src="`/anytype/icon/type/default/${typeDraft.icon || 'document'}.svg`"
                    alt=""
                    width="20"
                    height="20"
                    draggable="false"
                  />
                </button>
                <div v-if="iconPickerOpen" ref="iconPickerRef" class="icon-picker-menu">
                  <div class="icon-picker-head">
                    <div class="icon-picker-tab active">Иконки</div>
                  </div>
                  <div class="icon-picker-filter">
                    <input
                      ref="iconSearchRef"
                      class="icon-picker-search"
                      type="text"
                      placeholder="Отфильтровать"
                      v-model="iconFilter"
                    />
                  </div>
                  <div class="icon-picker-grid">
                    <button
                      v-for="name in filteredIcons"
                      :key="name"
                      class="icon-picker-item"
                      :class="{ active: name === typeDraft.icon }"
                      type="button"
                      :title="name"
                      @click="selectIcon(name)"
                    >
                      <img
                        :src="`/anytype/icon/type/default/${name}.svg`"
                        :alt="name"
                        width="24"
                        height="24"
                        :style="{ filter: `drop-shadow(0 0 0 ${typeDraft.color})` }"
                        draggable="false"
                      />
                    </button>
                  </div>
                </div>
              </div>
              <input
                class="type-editor-name-input"
                v-model="typeDraft.name"
                placeholder="например, Проект"
              />
            </div>
          </div>

          <div class="type-editor-section">
            <div class="type-editor-label">Тип во множественном числе</div>
            <div class="type-editor-name-row">
              <input
                class="type-editor-name-input"
                v-model="typeDraft.namePlural"
                placeholder="например, Проекты"
              />
            </div>
          </div>

          <!-- Color -->
          <div class="type-editor-section">
            <div class="type-editor-label">Цвет иконки</div>
            <div class="type-editor-color-row">
              <input type="color" v-model="typeDraft.color" class="settings-color-picker" />
              <input class="type-editor-color-input" v-model="typeDraft.color" />
            </div>
          </div>

          <!-- Layout -->
          <div class="type-editor-section">
            <div class="type-editor-label">Макет</div>
            <div class="type-editor-layout-items">
              <div class="type-editor-layout-row">
                <span>Тип макета</span>
                <select
                  class="settings-select-inline"
                  :value="headerTemplate.kind"
                  @change="
                    updateHeaderTemplate({
                      kind: ($event.target as HTMLSelectElement).value as HeaderLayoutKind,
                    })
                  "
                >
                  <option value="default">Обычный</option>
                  <option value="centered_profile">Портрет по центру</option>
                </select>
              </div>
            </div>
          </div>

          <!-- Fields -->
          <div class="type-editor-section">
            <div class="type-editor-section-title-row">
              <div class="type-editor-label">Свойства</div>
              <button class="settings-btn-secondary" type="button" @click="addField">+</button>
            </div>
            <div class="type-editor-fields">
              <div
                v-for="(field, index) in activeFields"
                :key="field.id"
                class="type-editor-field-row"
              >
                <input
                  class="type-editor-field-input"
                  :value="field.label"
                  placeholder="Название поля"
                  @input="updateFieldLabel(index, ($event.target as HTMLInputElement).value)"
                />
                <select
                  class="settings-select-inline"
                  :value="field.kind"
                  @change="
                    updateFieldKind(
                      index,
                      ($event.target as HTMLSelectElement).value as NoteFieldKind,
                    )
                  "
                >
                  <option value="text">Строка</option>
                  <option value="long_text">Длинный текст</option>
                  <option value="number">Число</option>
                  <option value="date">Дата</option>
                  <option value="boolean">Да / Нет</option>
                  <option value="select">Выбор</option>
                  <option value="image">Изображение</option>
                </select>
                <button class="settings-btn-danger-sm" type="button" @click="removeField(field.id)">
                  ×
                </button>
              </div>
            </div>
          </div>

          <!-- Delete -->
          <div v-if="typeDraft.id" class="type-editor-section">
            <button class="settings-btn-danger" type="button" @click="deleteType">
              Удалить тип
            </button>
          </div>

          <div v-if="typeError" class="dialog-error">{{ typeError }}</div>
        </div>
      </div>
      <div v-else class="type-editor-empty">Выберите тип объекта или создайте новый</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from "vue";
import {
  createDefaultHeaderTemplate,
  createDefaultNoteTypeDefinition,
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  type NoteTypeField,
} from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";

type HeaderLayoutKind = "default" | "centered_profile";

interface NoteTypeDraft {
  id?: string;
  name: string;
  namePlural: string;
  icon: string;
  color: string;
  schema_json: string;
  header_template_json: string;
}

const ICON_NAMES = [
  "accessibility",
  "add-circle",
  "airplane",
  "alarm",
  "albums",
  "alert-circle",
  "american-football",
  "analytics",
  "aperture",
  "apps",
  "archive",
  "attach",
  "backspace",
  "bag",
  "balloon",
  "ban",
  "bandage",
  "bar-chart",
  "barbell",
  "barcode",
  "baseball",
  "basket",
  "basketball",
  "beaker",
  "bed",
  "beer",
  "bicycle",
  "binoculars",
  "bluetooth",
  "boat",
  "body",
  "bonfire",
  "book",
  "bookmark",
  "bookmarks",
  "bowling-ball",
  "briefcase",
  "browsers",
  "brush",
  "bug",
  "build",
  "bulb",
  "bus",
  "business",
  "cafe",
  "calculator",
  "calendar",
  "calendar-clear",
  "calendar-number",
  "call",
  "camera",
  "camera-reverse",
  "car",
  "car-sport",
  "card",
  "cart",
  "cash",
  "cellular",
  "chatbox",
  "chatbox-ellipses",
  "chatbubble",
  "chatbubble-ellipses",
  "chatbubbles",
  "checkbox",
  "checkmark-circle",
  "checkmark-done-circle",
  "clipboard",
  "close-circle",
  "cloud",
  "cloud-circle",
  "cloud-done",
  "cloud-download",
  "cloud-offline",
  "cloud-upload",
  "cloudy",
  "cloudy-night",
  "code",
  "code-slash",
  "cog",
  "color-fill",
  "color-filter",
  "color-palette",
  "color-wand",
  "compass",
  "construct",
  "contract",
  "contrast",
  "copy",
  "create",
  "crop",
  "cube",
  "cut",
  "desktop",
  "diamond",
  "dice",
  "disc",
  "document",
  "document-attach",
  "document-lock",
  "document-text",
  "documents",
  "download",
  "duplicate",
  "ear",
  "earth",
  "easel",
  "egg",
  "ellipse",
  "enter",
  "exit",
  "expand",
  "extension-puzzle",
  "eye",
  "eye-off",
  "eyedrop",
  "fast-food",
  "female",
  "film",
  "filter-circle",
  "finger-print",
  "fish",
  "fitness",
  "flag",
  "flame",
  "flash",
  "flash-off",
  "flashlight",
  "flask",
  "flower",
  "folder",
  "folder-open",
  "football",
  "footsteps",
  "funnel",
  "game-controller",
  "gift",
  "git-branch",
  "git-commit",
  "git-compare",
  "git-merge",
  "git-network",
  "git-pull-request",
  "glasses",
  "globe",
  "golf",
  "grid",
  "hammer",
  "hand-left",
  "hand-right",
  "happy",
  "hardware-chip",
  "headset",
  "heart",
  "heart-circle",
  "heart-dislike",
  "heart-half",
  "help-buoy",
  "help-circle",
  "home",
  "hourglass",
  "ice-cream",
  "id-card",
  "image",
  "images",
  "infinite",
  "information-circle",
  "journal",
  "key",
  "keypad",
  "language",
  "laptop",
  "layers",
  "leaf",
  "library",
  "link",
  "list",
  "list-circle",
  "locate",
  "location",
  "lock-closed",
  "lock-open",
  "log-in",
  "log-out",
  "magnet",
  "mail",
  "mail-open",
  "mail-unread",
  "male",
  "man",
  "map",
  "medal",
  "medical",
  "medkit",
  "megaphone",
  "mic",
  "mic-circle",
  "moon",
  "move",
  "musical-note",
  "musical-notes",
  "navigate",
  "navigate-circle",
  "newspaper",
  "notifications",
  "notifications-circle",
  "nuclear",
  "nutrition",
  "options",
  "paper-plane",
  "partly-sunny",
  "pause",
  "pause-circle",
  "paw",
  "pencil",
  "people",
  "people-circle",
  "person",
  "person-add",
  "person-circle",
  "person-remove",
  "phone-landscape",
  "phone-portrait",
  "pie-chart",
  "pin",
  "pint",
  "pizza",
  "planet",
  "play",
  "play-circle",
  "podium",
  "power",
  "pricetag",
  "pricetags",
  "print",
  "prism",
  "pulse",
  "push",
  "qr-code",
  "radio",
  "rainy",
  "reader",
  "receipt",
  "recording",
  "refresh",
  "reload",
  "remove-circle",
  "repeat",
  "resize",
  "restaurant",
  "ribbon",
  "rocket",
  "rose",
  "sad",
  "save",
  "scale",
  "scan",
  "school",
  "search",
  "send",
  "server",
  "settings",
  "shapes",
  "share",
  "share-social",
  "shield",
  "shield-checkmark",
  "shield-half",
  "shirt",
  "shuffle",
  "skull",
  "snow",
  "sparkles",
  "speedometer",
  "square",
  "star",
  "star-half",
  "stats-chart",
  "stop",
  "stopwatch",
  "storefront",
  "subway",
  "sunny",
  "swap-horizontal",
  "swap-vertical",
  "sync",
  "tablet-landscape",
  "tablet-portrait",
  "telescope",
  "tennisball",
  "terminal",
  "text",
  "thermometer",
  "thumbs-down",
  "thumbs-up",
  "thunderstorm",
  "ticket",
  "time",
  "timer",
  "today",
  "toggle",
  "trail-sign",
  "train",
  "trash",
  "trash-bin",
  "trending-down",
  "trending-up",
  "triangle",
  "trophy",
  "tv",
  "umbrella",
  "unlink",
  "videocam",
  "volume-high",
  "volume-low",
  "volume-medium",
  "volume-mute",
  "walk",
  "wallet",
  "warning",
  "watch",
  "water",
  "wifi",
  "wine",
  "woman",
];

const builtInTypes = [
  { name: "Страница", icon: "document", color: "#2aa7ee" },
  { name: "Тренировка", icon: "barbell", color: "#f97316" },
  { name: "Упражнение", icon: "fitness", color: "#22c55e" },
];

const props = defineProps<{
  noteTypes: NoteType[];
  onNoteTypeSave: (
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;
      slug?: string;
    },
  ) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
}>();

const selectedTypeId = ref<string | null>(null);
const typeDraft = ref<NoteTypeDraft | null>(null);
const typeError = ref<string | null>(null);
const iconPickerOpen = ref(false);
const iconFilter = ref("");
const filterQuery = ref("");
const iconPickerRef = ref<HTMLDivElement | null>(null);
const iconSearchRef = ref<HTMLInputElement | null>(null);

const filteredTypes = computed(() => {
  const userTypes = props.noteTypes.filter((nt) => !isSystemType(nt.id));
  const q = filterQuery.value.trim().toLowerCase();
  if (!q) return userTypes;
  return userTypes.filter((nt) => nt.name.toLowerCase().includes(q));
});

const activeFields = computed<NoteTypeField[]>(() => {
  if (!typeDraft.value) return [];
  try {
    return parseNoteTypeDefinition(typeDraft.value.schema_json).fields;
  } catch {
    return [];
  }
});

const headerTemplate = computed(() => {
  if (!typeDraft.value)
    return parseHeaderTemplate(JSON.stringify(createDefaultHeaderTemplate("default")));
  return parseHeaderTemplate(typeDraft.value.header_template_json);
});

const filteredIcons = computed(() => {
  const q = iconFilter.value.toLowerCase();
  return q ? ICON_NAMES.filter((n) => n.includes(q)) : ICON_NAMES;
});

function newDraft(): NoteTypeDraft {
  return {
    name: "",
    namePlural: "",
    icon: "document",
    color: "#2aa7ee",
    schema_json: JSON.stringify(createDefaultNoteTypeDefinition()),
    header_template_json: JSON.stringify(createDefaultHeaderTemplate("default")),
  };
}

function openTypeEditor(noteType?: NoteType) {
  if (!noteType) {
    typeDraft.value = newDraft();
    selectedTypeId.value = null;
    typeError.value = null;
    return;
  }
  selectedTypeId.value = noteType.id;
  typeDraft.value = {
    id: noteType.id,
    name: noteType.name,
    namePlural: noteType.slug || noteType.name,
    icon: noteType.icon ?? "document",
    color: noteType.color ?? "#2aa7ee",
    schema_json: noteType.schema_json,
    header_template_json: noteType.header_template_json,
  };
  typeError.value = null;
}

function closeTypeEditor() {
  typeDraft.value = null;
  selectedTypeId.value = null;
  typeError.value = null;
}

function updateDraftFields(fields: NoteTypeField[]) {
  if (!typeDraft.value) return;
  typeDraft.value = { ...typeDraft.value, schema_json: JSON.stringify({ fields }) };
}

function updateHeaderTemplate(
  patch: Partial<{
    kind: HeaderLayoutKind;
    primaryFieldIds?: string[];
    secondaryFieldIds?: string[];
    imageFieldId?: string | null;
  }>,
) {
  if (!typeDraft.value) return;
  typeDraft.value = {
    ...typeDraft.value,
    header_template_json: JSON.stringify({ ...headerTemplate.value, ...patch }),
  };
}

function addField() {
  const fields = activeFields.value;
  updateDraftFields([
    ...fields,
    { id: `field_${fields.length + 1}`, label: "Новое поле", kind: "text", required: false },
  ]);
}

function removeField(fieldId: string) {
  updateDraftFields(activeFields.value.filter((f) => f.id !== fieldId));
}

function updateFieldLabel(index: number, value: string) {
  const next = [...activeFields.value];
  const field = next[index]!;
  next[index] = { ...field, label: value, id: value.trim().replace(/\s+/g, "_") || field.id };
  updateDraftFields(next);
}

function updateFieldKind(index: number, kind: NoteFieldKind) {
  const next = [...activeFields.value];
  next[index] = { ...next[index]!, kind };
  updateDraftFields(next);
}

function selectIcon(name: string) {
  if (!typeDraft.value) return;
  typeDraft.value = { ...typeDraft.value, icon: name };
  iconPickerOpen.value = false;
}

async function deleteType() {
  if (!typeDraft.value?.id) return;
  await props.onNoteTypeDelete(typeDraft.value.id);
  closeTypeEditor();
}

async function submitTypeEditor() {
  if (!typeDraft.value) return;
  const fields = activeFields.value;
  const tmpl = headerTemplate.value;
  const textFieldIds = fields
    .filter((f) => f.kind === "text" || f.kind === "long_text")
    .map((f) => f.id);
  const imageFieldId = fields.find((f) => f.kind === "image")?.id ?? null;
  const nextTemplate = {
    ...tmpl,
    imageFieldId: tmpl.imageFieldId ?? imageFieldId,
    primaryFieldIds: tmpl.primaryFieldIds?.length ? tmpl.primaryFieldIds : textFieldIds.slice(0, 2),
    secondaryFieldIds: tmpl.secondaryFieldIds?.length
      ? tmpl.secondaryFieldIds
      : textFieldIds.slice(2, 4),
  };
  const result = await props.onNoteTypeSave({
    id: typeDraft.value.id,
    name: typeDraft.value.name.trim(),
    slug: typeDraft.value.namePlural.trim() || typeDraft.value.name.trim(),
    icon: typeDraft.value.icon.trim() || null,
    color: typeDraft.value.color.trim() || null,
    schema_json: typeDraft.value.schema_json,
    header_template_json: JSON.stringify(nextTemplate),
  });
  if (!result.ok) {
    typeError.value =
      "message" in result ? (result as { message: string }).message : "Не удалось сохранить";
    return;
  }
  closeTypeEditor();
}

// Focus icon search when picker opens
watch(iconPickerOpen, (open) => {
  if (open) nextTick(() => iconSearchRef.value?.focus());
});

// Close icon picker on outside click / Escape
function handlePickerOutsideClick(e: MouseEvent) {
  if (iconPickerRef.value && !iconPickerRef.value.contains(e.target as Node)) {
    iconPickerOpen.value = false;
  }
}
function handlePickerEsc(e: KeyboardEvent) {
  if (e.key === "Escape") iconPickerOpen.value = false;
}
onMounted(() => {
  window.addEventListener("mousedown", handlePickerOutsideClick);
  window.addEventListener("keydown", handlePickerEsc);
});
onUnmounted(() => {
  window.removeEventListener("mousedown", handlePickerOutsideClick);
  window.removeEventListener("keydown", handlePickerEsc);
});
</script>
