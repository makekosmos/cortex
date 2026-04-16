import {
  Activity,
  BarChart3,
  ChevronDown,
  Clock,
  Gamepad2,
  Loader2,
} from "lucide-react";
import { type ComponentProps, useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { statsApi } from "@/lib/api";
import type { PlaytimeStats } from "@/types";

const toHours = (seconds: number) => Math.round((seconds / 3600) * 10) / 10;

// `Date#toISOString()` converts to UTC which can shift the calendar date depending
// on the local timezone (e.g. Jan 1 -> Dec 31). For date inputs + backend ranges
// we want a stable *local* YYYY-MM-DD string.
const toIsoDate = (value: Date) => {
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

const addDays = (value: Date, amount: number) => {
  const nextDate = new Date(value);
  nextDate.setDate(nextDate.getDate() + amount);
  return nextDate;
};

const formatDuration = (seconds: number) => {
  const totalMinutes = Math.round(seconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours <= 0) {
    return `${minutes} мин`;
  }

  return `${hours} ч ${minutes} мин`;
};

const formatDateShort = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "short",
  });
};

const formatDateLong = (value: string) => {
  const date = new Date(`${value}T00:00:00`);
  return date.toLocaleDateString("ru-RU", {
    day: "2-digit",
    month: "long",
  });
};

const formatDateMonthLabel = (value: string) => {
  const date = new Date(`${value}-01T00:00:00`);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleDateString("ru-RU", {
    year: "numeric",
    month: "long",
  });
};

const formatGameName = (name: string) =>
  name.length > 20 ? `${name.slice(0, 18)}…` : name;

const formatMonthValue = (year: number, month: number) =>
  `${year}-${String(month).padStart(2, "0")}`;

const getMonthRange = (value: string) => {
  const [yearValue, monthValue] = value.split("-");
  const year = Number(yearValue);
  const month = Number(monthValue);

  if (!year || !month) {
    return null;
  }

  const start = toIsoDate(new Date(year, month - 1, 1));
  const end = toIsoDate(new Date(year, month, 0));

  return { start, end };
};

const getMonthValueFromRange = (start: string, end: string) => {
  const startParts = start.split("-").map(Number);
  const endParts = end.split("-").map(Number);

  if (startParts.length !== 3 || endParts.length !== 3) {
    return "";
  }

  const [startYear, startMonth, startDay] = startParts;
  const [endYear, endMonth, endDay] = endParts;

  if (
    !startYear ||
    !startMonth ||
    !startDay ||
    !endYear ||
    !endMonth ||
    !endDay
  ) {
    return "";
  }

  if (startYear !== endYear || startMonth !== endMonth || startDay !== 1) {
    return "";
  }

  const lastDay = new Date(startYear, startMonth, 0).getDate();
  if (endDay !== lastDay) {
    return "";
  }

  return formatMonthValue(startYear, startMonth);
};

const buildRecentDateOptions = (today: Date, days: number) => {
  const options: string[] = [];

  for (let offset = 0; offset < days; offset++) {
    options.push(toIsoDate(addDays(today, -offset)));
  }

  return options;
};

const buildMonthOptions = (today: Date, months: number) => {
  const options: string[] = [];

  for (let offset = 0; offset < months; offset++) {
    const date = new Date(today.getFullYear(), today.getMonth() - offset, 1);
    options.push(formatMonthValue(date.getFullYear(), date.getMonth() + 1));
  }

  return options;
};

const rangePresets = [
  { id: "7d", label: `7 дней`, days: 7 },
  { id: "30d", label: `30 дней`, days: 30 },
  { id: "90d", label: `90 дней`, days: 90 },
];

const tooltipContentStyle = {
  backgroundColor: "hsl(var(--popover))",
  borderRadius: "12px",
  border: "1px solid hsl(var(--border))",
};

const tooltipLabelStyle = {
  color: "hsl(var(--foreground))",
  fontSize: 12,
};

const chartLineColor = "hsl(0 0% 100%)";

const tooltipItemStyle = {
  color: "hsl(var(--foreground))",
};

export default function Statistics() {
  const today = new Date();
  const defaultEnd = toIsoDate(today);
  const defaultStart = toIsoDate(addDays(today, -29));

  const [stats, setStats] = useState<PlaytimeStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [rangePreset, setRangePreset] = useState("30d");
  const [startDate, setStartDate] = useState(defaultStart);
  const [endDate, setEndDate] = useState(defaultEnd);
  const monthOptions = useMemo(() => buildMonthOptions(today, 24), [today]);
  const dayOptions = useMemo(() => buildRecentDateOptions(today, 365), [today]);

  useEffect(() => {
    let isActive = true;

    const loadStats = async () => {
      setLoading(true);
      setError(null);
      try {
        const response = await statsApi.getPlaytimeStats(startDate, endDate);
        if (!isActive) return;
        setStats(response);
      } catch (err) {
        console.error("Failed to load playtime stats:", err);
        if (!isActive) return;
        setError(
          "Не удалось загрузить статистику",
        );
      } finally {
        if (isActive) {
          setLoading(false);
        }
      }
    };

    loadStats();

    return () => {
      isActive = false;
    };
  }, [startDate, endDate]);

  const dailyData = useMemo(
    () =>
      stats?.daily_totals.map((entry) => ({
        date: entry.date,
        hours: toHours(entry.seconds),
        seconds: entry.seconds,
      })) ?? [],
    [stats],
  );

  const perGameData = useMemo(
    () =>
      stats?.per_game_totals.slice(0, 8).map((entry) => ({
        ...entry,
        hours: toHours(entry.seconds),
      })) ?? [],
    [stats],
  );

  const totalDays = stats?.daily_totals.length ?? 0;
  const activeDays =
    stats?.daily_totals.filter((entry) => entry.seconds > 0).length ?? 0;
  const averageSeconds =
    stats && totalDays > 0 ? Math.round(stats.total_seconds / totalDays) : 0;
  const rangeLabel = stats
    ? `${formatDateLong(stats.range_start)} — ${formatDateLong(
        stats.range_end,
      )}`
    : "";
  const topGame = stats?.per_game_totals[0];
  const hasDailyData = (stats?.total_seconds ?? 0) > 0;
  const hasPerGameData = perGameData.length > 0;
  const hasMoreGames =
    stats && stats.per_game_totals.length > perGameData.length;
  const selectedMonthValue = useMemo(
    () => getMonthValueFromRange(startDate, endDate),
    [startDate, endDate],
  );

  const handlePresetClick = (days: number, presetId: string) => {
    const now = new Date();
    const nextEnd = toIsoDate(now);
    const nextStart = toIsoDate(addDays(now, -(days - 1)));
    setRangePreset(presetId);
    setStartDate(nextStart);
    setEndDate(nextEnd);
  };

  const handleMonthChange = (value: string) => {
    const range = getMonthRange(value);
    if (!range) {
      return;
    }

    setRangePreset("month");
    setStartDate(range.start);
    setEndDate(range.end);
  };

  const handleStartDateChange = (value: string) => {
    setRangePreset("custom");
    setStartDate(value);
    if (value && value > endDate) {
      setEndDate(value);
    }
  };

  const handleEndDateChange = (value: string) => {
    setRangePreset("custom");
    setEndDate(value);
    if (value && value < startDate) {
      setStartDate(value);
    }
  };

  if (loading && !stats) {
    return (
      <div className="p-6 flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (!stats && error) {
    return (
      <div className="p-6 flex items-center justify-center h-full">
        <div className="text-sm text-destructive">{error}</div>
      </div>
    );
  }

  type TooltipFormatter = NonNullable<
    ComponentProps<typeof Tooltip>["formatter"]
  >;
  type TooltipLabelFormatter = NonNullable<
    ComponentProps<typeof Tooltip>["labelFormatter"]
  >;

  const tooltipFormatter: TooltipFormatter = (_value, _name, item) => {
    const payloadSeconds =
      typeof item === "object" && item && "payload" in item
        ? // Recharts doesn't expose a stable payload type here.
          // We only need the single optional field we control.
          (item as { payload?: { seconds?: number } }).payload?.seconds
        : undefined;

    return [
      formatDuration(payloadSeconds ?? 0),
      "Время",
    ] as [string, string];
  };

  const tooltipLabelFormatter: TooltipLabelFormatter = (label) =>
    formatDateLong(String(label ?? ""));

  return (
    <div className="p-4 sm:p-6 space-y-6">
      <div className="space-y-1">
        <h1 className="text-xl sm:text-2xl font-bold tracking-tight">
          {"Статистика"}
        </h1>
        <p className="text-sm text-muted-foreground">
          {
            "Обзор игровой активности за выбранный период"
          }
        </p>
      </div>

      <Card>
        <CardHeader className="space-y-1">
          <CardTitle className="flex items-center gap-2 text-base">
            <BarChart3 className="h-4 w-4 text-muted-foreground" />
            {"Период"}
          </CardTitle>
          <CardDescription>
            {rangeLabel ||
              "Выберите диапазон для отчета"}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap gap-2">
            {rangePresets.map((preset) => (
              <Button
                key={preset.id}
                size="sm"
                variant={rangePreset === preset.id ? "secondary" : "outline"}
                className="rounded-full"
                onClick={() => handlePresetClick(preset.days, preset.id)}
              >
                {preset.label}
              </Button>
            ))}
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            <div className="space-y-1">
              <label
                htmlFor="stats-month"
                className="text-xs text-foreground/90 font-medium"
              >
                {"Месяц"}
              </label>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    id="stats-month"
                    variant="outline"
                    className="w-full justify-between"
                    aria-label={`Выберите месяц: ${
                      selectedMonthValue
                        ? formatDateMonthLabel(selectedMonthValue)
                        : "не выбран"
                    }`}
                  >
                    <span>
                      {selectedMonthValue
                        ? formatDateMonthLabel(selectedMonthValue)
                        : "Выберите месяц"}
                    </span>
                    <ChevronDown className="h-3.5 w-3.5 text-foreground/60" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent className="w-full min-w-[220px]" align="start">
                  {monthOptions.map((value) => (
                    <DropdownMenuItem
                      key={value}
                      onSelect={() => handleMonthChange(value)}
                      className={
                        selectedMonthValue === value
                          ? "font-medium text-foreground"
                          : ""
                      }
                    >
                      {formatDateMonthLabel(value)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            <div className="space-y-1">
              <label
                htmlFor="stats-start-date"
                className="text-xs text-foreground/90 font-medium"
              >
                {"С"}
              </label>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    id="stats-start-date"
                    variant="outline"
                    className="w-full justify-between"
                    aria-label={`Выберите начальную дату: ${formatDateLong(
                      startDate,
                    )}`}
                  >
                    <span>{formatDateLong(startDate)}</span>
                    <ChevronDown className="h-3.5 w-3.5 text-foreground/60" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent className="w-full min-w-[220px]" align="start">
                  {dayOptions.map((value) => (
                    <DropdownMenuItem
                      key={`start-${value}`}
                      onSelect={() => handleStartDateChange(value)}
                      className={
                        startDate === value ? "font-medium text-foreground" : ""
                      }
                    >
                      {formatDateLong(value)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
            <div className="space-y-1">
              <label
                htmlFor="stats-end-date"
                className="text-xs text-foreground/90 font-medium"
              >
                {"По"}
              </label>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    id="stats-end-date"
                    variant="outline"
                    className="w-full justify-between"
                    aria-label={`Выберите конечную дату: ${formatDateLong(
                      endDate,
                    )}`}
                  >
                    <span>{formatDateLong(endDate)}</span>
                    <ChevronDown className="h-3.5 w-3.5 text-foreground/60" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent className="w-full min-w-[220px]" align="start">
                  {dayOptions.map((value) => (
                    <DropdownMenuItem
                      key={`end-${value}`}
                      onSelect={() => handleEndDateChange(value)}
                      className={endDate === value ? "font-medium text-foreground" : ""}
                    >
                      {formatDateLong(value)}
                    </DropdownMenuItem>
                  ))}
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
        </CardContent>
      </Card>

      {error ? (
        <Card className="border-destructive/40 bg-destructive/5">
          <CardContent className="py-4 text-sm text-destructive">
            {error}
          </CardContent>
        </Card>
      ) : null}

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card className="bg-card/60">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {
                "Всего за период"
              }
            </CardTitle>
            <Clock className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-semibold text-foreground">
              {stats ? formatDuration(stats.total_seconds) : "—"}
            </div>
            <p className="text-xs text-muted-foreground/90">
              {rangeLabel ||
                "За последний период"}
            </p>
          </CardContent>
        </Card>

        <Card className="bg-card/60">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {
                "Среднее в день"
              }
            </CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-semibold text-foreground">
              {stats ? formatDuration(averageSeconds) : "—"}
            </div>
            <p className="text-xs text-muted-foreground/90">
              {stats
                ? `${activeDays} из ${totalDays} дней были активными`
                : "Нет данных"}
            </p>
          </CardContent>
        </Card>

        <Card className="bg-card/60">
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {
                "Игры за период"
              }
            </CardTitle>
            <Gamepad2 className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-semibold text-foreground">
              {stats ? stats.per_game_totals.length : 0}
            </div>
            <p className="text-xs text-muted-foreground/90">
              {topGame
                ? `Топ: ${topGame.name} — ${formatDuration(
                    topGame.seconds,
                  )}`
                : "Нет активности"}
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-4">
        <Card className="bg-card/60">
          <CardHeader className="space-y-1">
            <CardTitle className="text-base flex items-center gap-2">
              <BarChart3 className="h-4 w-4 text-muted-foreground" />
              {
                "Динамика по дням"
              }
            </CardTitle>
            <CardDescription>{rangeLabel}</CardDescription>
          </CardHeader>
          <CardContent
            className="h-[260px]"
            role="img"
            aria-label={
              hasDailyData
                ? `Диаграмма динамики по дням: ${dailyData.length} точек`
                : "Нет данных за период"
            }
          >
            {hasDailyData ? (
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart
                  data={dailyData}
                  margin={{ top: 8, right: 16, left: 0, bottom: 0 }}
                >
                  <defs>
                    <linearGradient
                      id="dailyGradient"
                      x1="0"
                      y1="0"
                      x2="0"
                      y2="1"
                    >
                      <stop
                        offset="5%"
                        stopColor={chartLineColor}
                        stopOpacity={0.35}
                      />
                      <stop
                        offset="95%"
                        stopColor={chartLineColor}
                        stopOpacity={0}
                      />
                    </linearGradient>
                  </defs>
                  <CartesianGrid
                    strokeDasharray="3 3"
                    vertical={false}
                    stroke="hsl(var(--border))"
                  />
                  <XAxis
                    dataKey="date"
                    tickFormatter={formatDateShort}
                    tick={{
                      fill: "hsl(var(--foreground))",
                      fontSize: 11,
                    }}
                    tickLine={false}
                    axisLine={false}
                    minTickGap={16}
                  />
                  <YAxis
                    tick={{
                      fill: "hsl(var(--foreground))",
                      fontSize: 11,
                    }}
                    tickFormatter={(value: number) => `${value} ч`}
                    tickLine={false}
                    axisLine={false}
                  />
                  <Tooltip
                    contentStyle={tooltipContentStyle}
                    labelStyle={tooltipLabelStyle}
                    itemStyle={tooltipItemStyle}
                    cursor={{ fill: "transparent" }}
                    formatter={tooltipFormatter}
                    labelFormatter={tooltipLabelFormatter}
                  />
                  <Area
                    type="monotone"
                    dataKey="hours"
                    stroke={chartLineColor}
                    strokeWidth={2}
                    fill="url(#dailyGradient)"
                    dot={false}
                    activeDot={{ r: 4 }}
                  />
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
                {
                  "Нет данных за период"
                }
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="bg-card/60">
          <CardHeader className="space-y-1">
            <CardTitle className="text-base flex items-center gap-2">
              <Gamepad2 className="h-4 w-4 text-muted-foreground" />
              {
                "Разбивка по играм"
              }
            </CardTitle>
            <CardDescription>
              {hasMoreGames
                ? "Показаны топ-8"
                : "За выбранный период"}
            </CardDescription>
          </CardHeader>
          <CardContent
            className="h-[260px]"
            role="img"
            aria-label={
              hasPerGameData
                ? `Разбивка по играм: ${perGameData.length} игр`
                : "Нет данных по играм"
                }
              >
            {hasPerGameData ? (
              <ResponsiveContainer width="100%" height="100%">
                <BarChart
                  data={perGameData}
                  layout="vertical"
                  margin={{ top: 8, right: 16, left: 8, bottom: 0 }}
                >
                  <CartesianGrid
                    strokeDasharray="3 3"
                    horizontal={false}
                    stroke="hsl(var(--border))"
                  />
                  <XAxis
                    type="number"
                    tickFormatter={(value: number) => `${value} ч`}
                    tick={{
                      fill: "hsl(var(--foreground))",
                      fontSize: 11,
                    }}
                    tickLine={false}
                    axisLine={false}
                  />
                  <YAxis
                    type="category"
                    dataKey="name"
                    width={140}
                    tickFormatter={formatGameName}
                    tick={{
                      fill: "hsl(var(--foreground))",
                      fontSize: 11,
                    }}
                    tickLine={false}
                    axisLine={false}
                  />
                  <Tooltip
                    contentStyle={tooltipContentStyle}
                    labelStyle={tooltipLabelStyle}
                    itemStyle={tooltipItemStyle}
                    cursor={{ fill: "transparent" }}
                    formatter={tooltipFormatter}
                  />
                  <Bar
                    dataKey="hours"
                    fill={chartLineColor}
                    radius={[0, 6, 6, 0]}
                    barSize={18}
                  />
                </BarChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-full flex items-center justify-center text-sm text-muted-foreground">
                {
                  "Нет данных по играм"
                }
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
