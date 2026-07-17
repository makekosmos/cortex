import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import type {
  CoderActivityDay,
  CoderBreakdownItem,
  CoderDifficultyStats,
  CoderSummary,
  CodingSubmission,
} from "./types";

const CODING_SUBMISSION_TYPE_ID = "coding_submission_obj";
const CODING_PROFILE_TYPE_ID = "coding_profile_obj";

interface RawObjectRecord {
  id: string;
  propsJson?: unknown;
  deletedAt?: string | null;
}

type ArkSubscribe = (event: string, handler: (payload: unknown) => void) => () => void;
type ArkRequest = (operation: string, params: Record<string, unknown>) => Promise<unknown>;

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

function count(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : 0;
}

export function profileDifficultyStats(records: RawObjectRecord[]): CoderDifficultyStats {
  const profile = records.find((record) => {
    const props = objectValue(record.propsJson);
    return !record.deletedAt && props.source === "leetcode";
  });
  const props = objectValue(profile?.propsJson);
  const solved = objectValue(props.solved);
  const available = objectValue(props.available);
  return {
    easy: count(solved.easy),
    easyTotal: count(available.easy),
    medium: count(solved.medium),
    mediumTotal: count(available.medium),
    hard: count(solved.hard),
    hardTotal: count(available.hard),
    total: count(solved.all),
    available: count(available.all),
  };
}

export async function refreshLeetCodeStats(request: ArkRequest, load: () => Promise<void>) {
  // См. postmortems.md § 2026-07-18: local reload не создаёт профиль сложности.
  await request("integrations.sync_now", { provider: "leetcode" });
  await load();
}

export function useCoderStats() {
  const submissions = shallowRef<CodingSubmission[]>([]);
  const difficulties = shallowRef<CoderDifficultyStats>(profileDifficultyStats([]));
  const loading = shallowRef(true);
  const refreshing = shallowRef(false);
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
      const [records, profiles] = await Promise.all([
        window.kepler.ark.request<RawObjectRecord[]>("list_objects_by_type", {
          type_id: CODING_SUBMISSION_TYPE_ID,
        }),
        window.kepler.ark.request<RawObjectRecord[]>("list_objects_by_type", {
          type_id: CODING_PROFILE_TYPE_ID,
        }),
      ]);
      submissions.value = records
        .map(toSubmission)
        .filter((submission): submission is CodingSubmission => submission !== null)
        .sort((left, right) => Date.parse(right.submittedAt) - Date.parse(left.submittedAt));
      difficulties.value = profileDifficultyStats(profiles);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось загрузить статистику";
      submissions.value = [];
      difficulties.value = profileDifficultyStats([]);
    } finally {
      loading.value = false;
    }
  }

  async function refresh(): Promise<void> {
    refreshing.value = true;
    error.value = "";
    try {
      await refreshLeetCodeStats(window.kepler.ark.request, load);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось обновить LeetCode";
    } finally {
      refreshing.value = false;
    }
  }

  function scheduleRefresh(payload: unknown): void {
    const typeId = objectValue(payload).type_id;
    if (typeId !== CODING_SUBMISSION_TYPE_ID && typeId !== CODING_PROFILE_TYPE_ID) return;
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

  return {
    activity,
    difficulties,
    error,
    languages,
    loading,
    recent,
    refreshing,
    statuses,
    summary,
    load,
    refresh,
  };
}
