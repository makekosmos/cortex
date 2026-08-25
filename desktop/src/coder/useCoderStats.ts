import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import type { Ref } from "vue";
import type {
  CoderPlatform,
  CoderActivityDay,
  CoderBreakdownItem,
  CoderDifficultyStats,
  CoderSummary,
  CodewarsProfileStats,
  CodingSubmission,
} from "./types";
import { isNumber, isRecord, isString } from "../shared/runtimeGuards";

const CODING_SUBMISSION_TYPE_ID = "coding_submission_obj";
const CODING_PROFILE_TYPE_ID = "coding_profile_obj";

interface RawObjectRecord {
  id: string;
  propsJson?: unknown;
  deletedAt?: string | null;
}

type ArkSubscribe = <T>(event: string, handler: (payload: T) => void) => () => void;
type ArkRequest = <T>(operation: string, params: { [key: string]: string }) => Promise<T>;
type JsonRecord = import("../shared/runtimeGuards").JsonRecord;
interface StreakStats {
  current: number;
  longest: number;
}

function objectValue<T>(value: T): JsonRecord {
  // SAFETY: the runtime record guard validates the bridge payload before narrowing.
  return isRecord(value) && !Array.isArray(value)
    ? (value as JsonRecord)
    : {};
}

function text<T>(value: T): string {
  return isString(value) ? value : "";
}

function toSubmission(record: RawObjectRecord): CodingSubmission | null {
  if (record.deletedAt) return null;
  const props = objectValue(record.propsJson);
  const source = text(props.source);
  if (source !== "leetcode" && source !== "codewars") return null;
  const submittedAt = text(props.submittedAt);
  if (!submittedAt || Number.isNaN(Date.parse(submittedAt))) return null;
  const languages = Array.isArray(props.languages)
    ? props.languages.filter(isString).map(String).filter(Boolean)
    : [];
  const language = text(props.language) || "Неизвестно";
  const rank = objectValue(props.rank);
  return {
    id: record.id,
    source,
    username: text(props.username),
    problemTitle: text(props.problemTitle) || "Задача",
    problemSlug: text(props.problemSlug),
    problemNumber: text(props.problemNumber),
    status: text(props.status) || "Неизвестно",
    accepted: props.accepted === true,
    language,
    languages: languages.length > 0 ? languages : [language],
    runtime: text(props.runtime),
    memory: text(props.memory),
    rankName: text(rank.name),
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

export function streaks(activeDays: Set<string>): StreakStats {
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

export function kyuBreakdown(values: string[]): CoderBreakdownItem[] {
  const ranks = values.filter((value) => /^[1-8] kyu$/.test(value));
  return breakdown(ranks, ranks.length).sort(
    (left, right) => Number.parseInt(right.label) - Number.parseInt(left.label),
  );
}

function count<T>(value: T): number {
  return isNumber(value) && Number.isFinite(value) && value >= 0 ? value : 0;
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

export function codewarsProfileStats(records: RawObjectRecord[]): CodewarsProfileStats {
  const profile = records.find((record) => {
    const props = objectValue(record.propsJson);
    return !record.deletedAt && props.source === "codewars";
  });
  const props = objectValue(profile?.propsJson);
  const rank = objectValue(props.rank);
  return {
    username: text(props.username),
    honor: count(props.honor),
    leaderboardPosition: count(props.leaderboardPosition),
    rankName: text(rank.name),
    rankScore: count(rank.score),
  };
}

export async function refreshCoderStats(
  request: ArkRequest,
  provider: CoderPlatform,
  load: () => Promise<void>,
) {
  // См. postmortems.md § 2026-07-18: local reload не создаёт профиль сложности.
  await request("integrations.sync_now", { provider });
  await load();
}

export function useCoderStats(platform: Ref<CoderPlatform>) {
  const allSubmissions = shallowRef<CodingSubmission[]>([]);
  const profiles = shallowRef<RawObjectRecord[]>([]);
  const loading = shallowRef(true);
  const refreshing = shallowRef(false);
  const error = shallowRef("");
  let unsubscribe: (() => void) | null = null;
  let refreshTimer: ReturnType<typeof setTimeout> | null = null;

  const difficulties = computed(() => profileDifficultyStats(profiles.value));
  const codewarsProfile = computed(() => codewarsProfileStats(profiles.value));
  const submissions = computed(() =>
    allSubmissions.value.filter(
      (submission) =>
        submission.source === platform.value &&
        (platform.value !== "codewars" ||
          submission.username.toLocaleLowerCase() ===
            codewarsProfile.value.username.toLocaleLowerCase()),
    ),
  );

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
      submissions.value.flatMap((submission) => submission.languages),
      submissions.value.reduce((total, submission) => total + submission.languages.length, 0),
    ),
  );
  const codewarsRanks = computed(() =>
    kyuBreakdown(submissions.value.map((submission) => submission.rankName)),
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
      const [records, profileRecords] = await Promise.all([
        window.kepler.ark.request<RawObjectRecord[]>("list_objects_by_type", {
          type_id: CODING_SUBMISSION_TYPE_ID,
        }),
        window.kepler.ark.request<RawObjectRecord[]>("list_objects_by_type", {
          type_id: CODING_PROFILE_TYPE_ID,
        }),
      ]);
      allSubmissions.value = records
        .map(toSubmission)
        .filter((submission): submission is CodingSubmission => submission !== null)
        .sort((left, right) => Date.parse(right.submittedAt) - Date.parse(left.submittedAt));
      profiles.value = profileRecords;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось загрузить статистику";
      allSubmissions.value = [];
      profiles.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function refresh(): Promise<void> {
    refreshing.value = true;
    error.value = "";
    try {
      await refreshCoderStats(window.kepler.ark.request, platform.value, load);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось обновить статистику";
    } finally {
      refreshing.value = false;
    }
  }

  function scheduleRefresh<T>(payload: T): void {
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
// SAFETY: the surrounding domain validation preserves the asserted contract.
    // SAFETY: the preload bridge is installed on the desktop window before mount.
    // SAFETY: the preload bridge is installed on the desktop window before mount.
    const subscribe = (
      window.kepler.ark as typeof window.kepler.ark & { subscribe?: ArkSubscribe }
    ).subscribe;
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
    codewarsProfile,
    codewarsRanks,
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
