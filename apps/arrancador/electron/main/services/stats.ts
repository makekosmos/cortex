import type { PlaytimeStats, PlaytimeStatsRepository } from "./contracts";
import {
  enumerateUtcDates,
  normalizeDateRange,
} from "./helpers/date-range";

export interface StatsService {
  getPlaytimeStats(start?: string, end?: string): Promise<PlaytimeStats>;
}

export function createStatsService(
  repository: PlaytimeStatsRepository,
): StatsService {
  return {
    async getPlaytimeStats(start?: string, end?: string) {
      const range = normalizeDateRange(start, end);
      const { dailyTotals, perGameTotals } = await repository.getRangeStats(
        range.rangeStart,
        range.rangeEnd,
      );

      const dailyMap = new Map<string, number>(
        dailyTotals.map((entry) => [entry.date, entry.seconds]),
      );

      const dailySeries = enumerateUtcDates(range.startDate, range.endDate).map(
        (date) => {
          const dateKey = date
            .toISOString()
            .slice(0, 10);
          return {
            date: dateKey,
            seconds: dailyMap.get(dateKey) ?? 0,
          };
        },
      );

      const orderedGameTotals = [...perGameTotals].sort(
        (a, b) => b.seconds - a.seconds,
      );

      return {
        range_start: range.rangeStart,
        range_end: range.rangeEnd,
        total_seconds: dailySeries.reduce((sum, entry) => sum + entry.seconds, 0),
        daily_totals: dailySeries,
        per_game_totals: orderedGameTotals,
      };
    },
  };
}
