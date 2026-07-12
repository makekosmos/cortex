<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useVirtualizer } from "@tanstack/vue-virtual";
import { Check, ChevronRight, CircleAlert, FileCode2, Terminal, X } from "@lucide/vue";
import type {
  AgentsApproval,
  AgentsRequestUserInputParams,
  AgentsTimelineEvent,
  AgentsUserInputAnswers,
} from "@kosmos/ark/agents";

const props = defineProps<{
  events: AgentsTimelineEvent[];
  approval: AgentsApproval | null;
  hasOlder: boolean;
  loadingOlder: boolean;
}>();
const emit = defineEmits<{
  loadOlder: [];
  respond: [approvalId: string, decision: "accept" | "decline", answers?: AgentsUserInputAnswers];
}>();
const answers = reactive<Record<string, string>>({});
const scroller = ref<HTMLElement | null>(null);
const virtualizer = useVirtualizer(
  computed(() => ({
    count: props.events.length,
    getScrollElement: () => scroller.value,
    estimateSize: () => 104,
    overscan: 8,
  })),
);
const rows = computed(() => virtualizer.value.getVirtualItems());
function title(kind: string): string {
  return (
    (
      {
        user_message: "Вы",
        message_delta: "Codex",
        item_started: "Действие",
        item_completed: "Завершено",
        reasoning_delta: "Сводка рассуждений",
        plan: "План",
        diff: "Изменения",
        command_output: "Вывод команды",
        file_change_delta: "Изменение файла",
        error: "Ошибка",
        approval: "Требуется подтверждение",
        question: "Вопрос",
        turn_completed: "Задача завершена",
      } as Record<string, string>
    )[kind] ?? kind
  );
}
function text(payload: unknown): string {
  if (typeof payload === "string") return payload;
  if (!payload || typeof payload !== "object") return "";
  const p = payload as Record<string, unknown>;
  return String(p.text ?? p.delta ?? p.message ?? p.content ?? JSON.stringify(payload, null, 2));
}
function icon(kind: string) {
  if (kind.includes("command") || kind === "item_started") return Terminal;
  if (kind.includes("diff") || kind.includes("file")) return FileCode2;
  if (kind === "error") return CircleAlert;
  return ChevronRight;
}
const questions = computed(() => {
  if (props.approval?.method !== "item/tool/requestUserInput") return [];
  const params = props.approval.params as AgentsRequestUserInputParams;
  return params.questions ?? [];
});
const canSubmitAnswers = computed(() =>
  questions.value.every((question) => Boolean(answers[question.id]?.trim())),
);
function submitAnswers(): void {
  if (!props.approval) return;
  const payload = Object.fromEntries(
    questions.value.map((question) => [question.id, { answers: [answers[question.id] ?? ""] }]),
  );
  emit("respond", props.approval.id, "accept", payload);
}
</script>

<template>
  <div ref="scroller" class="timeline">
    <div v-if="hasOlder" class="timeline__history">
      <button class="ghost" type="button" :disabled="loadingOlder" @click="emit('loadOlder')">
        {{ loadingOlder ? "Загружаем…" : "Показать ранние события" }}
      </button>
    </div>
    <div class="timeline__spacer" :style="{ height: `${virtualizer.getTotalSize()}px` }">
      <article
        v-for="row in rows"
        :key="props.events[row.index]?.id"
        class="event-card"
        :class="`event-card--${props.events[row.index]?.kind}`"
        :style="{ transform: `translateY(${row.start}px)` }"
        :data-index="row.index"
        ref="virtualizer.measureElement"
      >
        <header>
          <component :is="icon(props.events[row.index]?.kind ?? '')" :size="15" /><strong>{{
            title(props.events[row.index]?.kind ?? "")
          }}</strong
          ><span v-if="props.events[row.index]?.truncated" class="chip">вывод сокращён</span>
        </header>
        <pre>{{ text(props.events[row.index]?.payload) }}</pre>
      </article>
    </div>
    <article v-if="approval" class="approval-card">
      <header>
        <CircleAlert :size="17" /><strong>{{
          approval.method === "item/tool/requestUserInput"
            ? "Codex ждёт ответа"
            : "Требуется разрешение"
        }}</strong>
      </header>
      <div v-if="questions.length" class="approval-questions">
        <fieldset v-for="question in questions" :key="question.id">
          <legend>
            <span v-if="question.header">{{ question.header }}</span
            >{{ question.question }}
          </legend>
          <div v-if="question.options?.length" class="approval-options">
            <label v-for="option in question.options" :key="option.label" class="approval-option">
              <input
                v-model="answers[question.id]"
                type="radio"
                :name="`question-${question.id}`"
                :value="option.label"
              />
              <span
                ><strong>{{ option.label }}</strong
                ><small v-if="option.description">{{ option.description }}</small></span
              >
            </label>
          </div>
          <input
            v-else
            v-model="answers[question.id]"
            type="text"
            :aria-label="question.question"
          />
        </fieldset>
      </div>
      <pre v-else>{{ JSON.stringify(approval.params, null, 2) }}</pre>
      <footer>
        <button type="button" class="secondary" @click="emit('respond', approval.id, 'decline')">
          <X :size="15" />Отклонить</button
        ><button
          v-if="questions.length"
          type="button"
          class="primary"
          :disabled="!canSubmitAnswers"
          @click="submitAnswers"
        >
          <Check :size="15" />Ответить</button
        ><button
          v-else
          type="button"
          class="primary"
          @click="emit('respond', approval.id, 'accept')"
        >
          <Check :size="15" />Разрешить
        </button>
      </footer>
    </article>
  </div>
</template>
