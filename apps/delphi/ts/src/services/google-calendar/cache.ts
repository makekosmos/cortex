type CoverageRange = {
  coverageStart: string | null;
  coverageEnd: string | null;
  lastSyncAt: string | null;
};

type RefreshDecisionArgs = CoverageRange & {
  requestedStart: string;
  requestedEnd: string;
  now?: number;
  staleAfterMs?: number;
};

export function isRangeCovered(
  coverageStart: string | null,
  coverageEnd: string | null,
  requestedStart: string,
  requestedEnd: string,
): boolean {
  if (!coverageStart || !coverageEnd) return false;
  return coverageStart <= requestedStart && coverageEnd >= requestedEnd;
}

export function expandRequestedRange(
  requestedStart: string,
  requestedEnd: string,
  paddingDays = 14,
) {
  const start = new Date(requestedStart);
  const end = new Date(requestedEnd);

  start.setDate(start.getDate() - paddingDays);
  end.setDate(end.getDate() + paddingDays);

  return {
    start: start.toISOString(),
    end: end.toISOString(),
  };
}

export function shouldRefreshCoverage({
  coverageStart,
  coverageEnd,
  requestedStart,
  requestedEnd,
  lastSyncAt,
  now = Date.now(),
  staleAfterMs = 15 * 60 * 1000,
}: RefreshDecisionArgs): boolean {
  if (!isRangeCovered(coverageStart, coverageEnd, requestedStart, requestedEnd)) {
    return true;
  }

  if (!lastSyncAt) return true;

  const syncedAt = Date.parse(lastSyncAt);
  if (!Number.isFinite(syncedAt)) return true;

  return now - syncedAt > staleAfterMs;
}
