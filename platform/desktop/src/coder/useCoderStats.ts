import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import type { CoderActivityDay, CoderBreakdownItem, CoderSummary, CodingSubmission } from "./types";

const CODING_SUBMISSION_TYPE_ID = "coding_submission_obj";

interface RawObjectRecord {
  id: string;
  propsJson?: unknown;
  deletedAt?: string | null;
}

type ArkSubscribe = (event: string, handler: (payload: unknown) => void) => () => void;

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function text(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function toSubmission(record: RawObjectRecord): CodingSubmission | null {
  if (record.deletedAt) return null;
  const props = objectValue(record.propsJson);
  const submittedAt = text(props.submittedAt);
  if (!submittedAt || Number.isNaN(Date.parse(submittedAt))) return null;
  return {
    id: record.id,
    problemTitle: text(props.problemTitle) || "Задача",
    problemSlug: text(props.problemSlug),
    status: text(props.status) || "Неизвестно",
    accepted: props.accepted === true,
    language: text(props.language) || "Неизвестно",
    runtime: text(props.runtime),
    memory: text(props.memory),
    submittedAt,
    url: text(props.url),
  };
}

export function localDayKey(value: Date): string {
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function addLocalDays(value: Date, days: number): Date {
  return new Date(value.getFullYear(), value.getMonth(), value.getDate() + days);
}

export function streaks(activeDays: Set<string>): { current: number; longest: number } {
  if (activeDays.size === 0) return { current: 0, longest: 0 };
  const sorted = [...activeDays].sort();
  let longest = 1;
  let running = 1;
  for (let index = 1; index < sorted.length; index += 1) {
    const previous = new Date(`${sorted[index - 1]}T00:00:00`);
    const expected = localDayKey(addLocalDays(previous, 1));
    running = sorted[index] === expected ? running + 1 : 1;
    longest = Math.max(longest, running);
  }

  const today = new Date();
  const start = activeDays.has(localDayKey(today)) ? today : addLocalDays(today, -1);
  let current = 0;
  for (let day = start; activeDays.has(localDayKey(day)); day = addLocalDays(day, -1)) {
    current += 1;
  }
  return { current, longest };
}

export function breakdown(values: string[], total: number): CoderBreakdownItem[] {
  const counts = new Map<string, number>();
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
  return [...counts.entries()]
    .map(([label, count]) => ({ label, count, share: total > 0 ? count / total : 0 }))
    .sort((left, right) => right.count - left.count || left.label.localeCompare(right.label, "ru"));
}

export function useCoderStats() {
  const submissions = shallowRef<CodingSubmission[]>([]);
  const loading = shallowRef(true);
  const error = shallowRef("");
  let unsubscribe: (() => void) | null = null;
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;

  const activeDays = computed(
    () =>
      new Set(submissions.value.map((submission) => localDayKey(new Date(submission.submittedAt)))),
  );
  const streak = computed(() => streaks(activeDays.value));
  const summary = computed<CoderSummary>(() => {
    const accepted = submissions.value.filter((submission) => submission.accepted);
    const solved = new Set(
      accepted.map((submission) => submission.problemSlug || submission.problemTitle),
    ).size;
    return {
      submissions: submissions.value.length,
      accepted: accepted.length,
      solved,
      acceptanceRate: submissions.value.length > 0 ? accepted.length / submissions.value.length : 0,
      currentStreak: streak.value.current,
      longestStreak: streak.value.longest,
    };
  });
  const languages = computed(() =>
    breakdown(
      submissions.value.map((submission) => submission.language),
      submissions.value.length,
    ),
  );
  const statuses = computed(() =>
    breakdown(
      submissions.value.map((submission) => submission.status),
      submissions.value.length,
    ),
  );
  const activity = computed<CoderActivityDay[]>(() => {
    const counts = new Map<string, number>();
    for (const submission of submissions.value) {
      const key = localDayKey(new Date(submission.submittedAt));
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    const end = new Date();
    const start = addLocalDays(end, -370);
    const max = Math.max(1, ...counts.values());
    return Array.from({ length: 371 }, (_, index) => {
      const date = localDayKey(addLocalDays(start, index));
      const count = counts.get(date) ?? 0;
      return { date, count, level: count === 0 ? 0 : Math.max(1, Math.ceil((count / max) * 4)) };
    });
  });
  const recent = computed(() => submissions.value.slice(0, 40));

  async function load(): Promise<void> {
    loading.value = true;
    error.value = "";
    try {
      const records = await window.kepler.ark.request<RawObjectRecord[]>("list_objects_by_type", {
        type_id: CODING_SUBMISSION_TYPE_ID,
      });
      submissions.value = records
        .map(toSubmission)
        .filter((submission): submission is CodingSubmission => submission !== null)
        .sort((left, right) => Date.parse(right.submittedAt) - Date.parse(left.submittedAt));
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось загрузить статистику";
      submissions.value = [];
    } finally {
      loading.value = false;
    }
  }

  function scheduleRefresh(payload: unknown): void {
    const typeId = objectValue(payload).type_id;
    if (typeId !== CODING_SUBMISSION_TYPE_ID) return;
    if (refreshTimer) clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      refreshTimer = null;
      void load();
    }, 250);
  }

  onMounted(() => {
    void load();
    const subscribe = (window.kepler.ark as unknown as { subscribe?: ArkSubscribe }).subscribe;
    if (subscribe) {
      const offUpsert = subscribe("object_upserted", scheduleRefresh);
      const offDeleted = subscribe("object_deleted", scheduleRefresh);
      unsubscribe = () => {
        offUpsert();
        offDeleted();
      };
    }
  });

  onBeforeUnmount(() => {
    unsubscribe?.();
    if (refreshTimer) clearTimeout(refreshTimer);
  });

  return { activity, error, languages, loading, recent, statuses, summary, load };
}
