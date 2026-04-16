import {
  Cpu,
  HardDrive,
  Loader2,
  MemoryStick,
  Monitor,
  RefreshCw,
  Video,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { systemApi } from "@/lib/api";
import type { DiskSpeedResult, SystemInfo } from "@/types";

const STORAGE_KEY = "arrancador_system_info_snapshot";
const STORAGE_CHECKED_KEY = "arrancador_system_info_checked_at";

const formatBytes = (value: number) => {
  if (!value || value <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  const digits = size >= 10 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
};

const formatSpeed = (value: number) => `${Math.round(value)} MB/s`;

const formatUptime = (seconds: number) => {
  if (!seconds || seconds <= 0) {
    return "—";
  }

  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];

  if (days > 0) parts.push(`${days} д`);
  if (hours > 0 || days > 0) parts.push(`${hours} ч`);
  parts.push(`${minutes} мин`);

  return parts.join(" ");
};

const formatCheckedAt = (value: string | null) => {
  if (!value) return "—";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return "—";
  return parsed.toLocaleString("ru-RU");
};

const formatCoreLabel = (physical: number | null, logical: number) => {
  if (!logical) {
    return "—";
  }
  if (physical) {
    return `${physical} физ. / ${logical} лог.`;
  }
  return `${logical} лог.`;
};

const InfoRow = ({ label, value }: { label: string; value: string }) => (
  <div className="flex items-center justify-between gap-4 text-sm">
    <span className="text-muted-foreground">{label}</span>
    <span className="font-medium text-right break-words">{value}</span>
  </div>
);

export default function SystemInfoPage() {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [checkedAt, setCheckedAt] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [diskTests, setDiskTests] = useState<
    Record<
      string,
      { loading: boolean; result?: DiskSpeedResult; error?: string }
    >
  >({});

  useEffect(() => {
    if (typeof window === "undefined") return;

    const cached = window.localStorage.getItem(STORAGE_KEY);
    const cachedCheckedAt = window.localStorage.getItem(STORAGE_CHECKED_KEY);

    if (cached) {
      try {
        const parsed = JSON.parse(cached) as SystemInfo;
        setInfo(parsed);
      } catch (err) {
        console.warn("Failed to parse cached system info:", err);
        window.localStorage.removeItem(STORAGE_KEY);
      }
    }

    if (cachedCheckedAt) {
      setCheckedAt(cachedCheckedAt);
    }
  }, []);

  const handleRefresh = async () => {
    if (loading) return;

    setLoading(true);
    setError(null);

    try {
      const data = await systemApi.getInfo();
      const timestamp = new Date().toISOString();
      setInfo(data);
      setCheckedAt(timestamp);

      if (typeof window !== "undefined") {
        window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
        window.localStorage.setItem(STORAGE_CHECKED_KEY, timestamp);
      }
    } catch (err) {
      console.error("Failed to load system info:", err);
      setError(
        "Не удалось загрузить данные о системе",
      );
    } finally {
      setLoading(false);
    }
  };

  const handleDiskTest = async (mountPoint: string) => {
    setDiskTests((prev) => ({
      ...prev,
      [mountPoint]: { loading: true },
    }));

    try {
      const result = await systemApi.testDiskSpeed(mountPoint);
      setDiskTests((prev) => ({
        ...prev,
        [mountPoint]: { loading: false, result },
      }));
    } catch (err) {
      console.error("Failed to test disk speed:", err);
      setDiskTests((prev) => ({
        ...prev,
        [mountPoint]: {
          loading: false,
          error:
            "Не удалось провести тест",
        },
      }));
    }
  };

  const memoryUsagePercent = useMemo(() => {
    if (!info || info.memory.total_bytes <= 0) return 0;
    return Math.min(
      100,
      (info.memory.used_bytes / info.memory.total_bytes) * 100,
    );
  }, [info]);

  const diskCards = useMemo(() => {
    if (!info) return [];
    return info.disks.map((disk, index) => {
      const usedBytes = Math.max(0, disk.total_bytes - disk.available_bytes);
      const percent =
        disk.total_bytes > 0
          ? Math.min(100, (usedBytes / disk.total_bytes) * 100)
          : 0;
      const model = disk.model || disk.name || "—";
      const typeLabel = disk.media_type || disk.kind || "—";

      return {
        key: `${disk.mount_point}-${index}`,
        mountPoint: disk.mount_point,
        model,
        typeLabel,
        used: formatBytes(usedBytes),
        total: formatBytes(disk.total_bytes),
        free: disk.available_bytes
          ? formatBytes(disk.available_bytes)
          : "—",
        percent,
        isRemovable: disk.is_removable,
      };
    });
  }, [info]);

  const gpuItems = useMemo(() => {
    if (!info) return [];
    return info.gpus.map((gpu, index) => ({
      key: `${gpu.device_name}-${index}`,
      name:
        gpu.name ||
        "Видеокарта",
      isPrimary: gpu.is_primary,
    }));
  }, [info]);

  const monitorItems = useMemo(() => {
    if (!info) return [];
    return info.monitors.map((monitor, index) => {
      const name =
        monitor.name ||
        monitor.device_name ||
        `Монитор ${index + 1}`;
      const resolution =
        monitor.width && monitor.height
          ? `${monitor.width}×${monitor.height}`
          : "—";
      const refresh = monitor.refresh_rate
        ? `${monitor.refresh_rate} Гц`
        : "";
      return {
        key: `${monitor.device_name}-${index}`,
        name,
        resolution,
        refresh,
        isPrimary: monitor.is_primary,
      };
    });
  }, [info]);

  return (
    <div className="p-4 sm:p-6 space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-1">
          <h1 className="text-xl sm:text-2xl font-bold tracking-tight">
            {"Система"}
          </h1>
          <p className="text-sm text-muted-foreground">
            {
              "Ключевые параметры вашего ПК"
            }
          </p>
          <p className="text-xs text-muted-foreground">
            {`Последняя проверка: ${formatCheckedAt(checkedAt)}`}
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          className="rounded-full"
          onClick={handleRefresh}
          disabled={loading}
        >
          {loading ? (
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="mr-2 h-4 w-4" />
          )}
          {
            "Проверить снова"
          }
        </Button>
      </div>

      {error ? (
        <Card className="border-destructive/40 bg-destructive/5">
          <CardContent className="py-4 text-sm text-destructive">
            {error}
          </CardContent>
        </Card>
      ) : null}

      {!info && !loading ? (
        <Card className="bg-card/60">
          <CardContent className="py-6 text-sm text-muted-foreground">
            {
              "Данных пока нет. Нажмите «Проверить снова», чтобы обновить показатели."
            }
          </CardContent>
        </Card>
      ) : null}

      {info ? (
        <div className="space-y-4">
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
            <Card className="bg-card/60">
              <CardHeader className="space-y-1">
                <CardTitle className="text-base flex items-center gap-2">
                  <Cpu className="h-4 w-4 text-muted-foreground" />
                  {"Процессор"}
                </CardTitle>
                <CardDescription>
                  {info.cpu.vendor_id || "—"}
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <InfoRow
                  label={"Модель"}
                  value={info.cpu.brand || "—"}
                />
                <InfoRow
                  label={"Ядра"}
                  value={formatCoreLabel(
                    info.cpu.physical_cores,
                    info.cpu.logical_cores,
                  )}
                />
                <InfoRow
                  label={"Частота"}
                  value={
                    info.cpu.frequency_mhz
                      ? `${info.cpu.frequency_mhz} MHz`
                      : "—"
                  }
                />
              </CardContent>
            </Card>

            <Card className="bg-card/60">
              <CardHeader className="space-y-1">
                <CardTitle className="text-base flex items-center gap-2">
                  <Video className="h-4 w-4 text-muted-foreground" />
                  {
                    "Видеокарта"
                  }
                </CardTitle>
                <CardDescription>
                  {gpuItems.length > 1
                    ? `Найдено ${gpuItems.length}`
                    : "Основной адаптер"}
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-2">
                {gpuItems.length > 0 ? (
                  gpuItems.map((gpu) => (
                    <div
                      key={gpu.key}
                      className="flex items-center justify-between gap-4 text-sm"
                    >
                      <span className="font-medium">{gpu.name}</span>
                      <span className="text-xs text-muted-foreground">
                        {gpu.isPrimary
                          ? "Основная"
                          : ""}
                      </span>
                    </div>
                  ))
                ) : (
                  <div className="text-sm text-muted-foreground">
                    {"Не найдена"}
                  </div>
                )}
              </CardContent>
            </Card>
          </div>

          <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
            <Card className="bg-card/60">
              <CardHeader className="space-y-1">
                <CardTitle className="text-base flex items-center gap-2">
                  <MemoryStick className="h-4 w-4 text-muted-foreground" />
                  {"ОЗУ"}
                </CardTitle>
                <CardDescription>
                  {
                    "Объем и загрузка памяти"
                  }
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-muted-foreground">
                      {
                        "Используется"
                      }
                    </span>
                    <span className="font-medium">
                      {`${formatBytes(info.memory.used_bytes)} / ${formatBytes(
                        info.memory.total_bytes,
                      )}`}
                    </span>
                  </div>
                  <Progress value={memoryUsagePercent} />
                  <div className="flex items-center justify-between text-xs text-muted-foreground">
                    <span>
                      {`Доступно ${formatBytes(
                        info.memory.available_bytes,
                      )}`}
                    </span>
                    <span>
                      {`Свободно ${formatBytes(
                        info.memory.free_bytes,
                      )}`}
                    </span>
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div className="rounded-lg border border-border/60 bg-card/40 p-3">
                    <div className="text-xs text-muted-foreground">
                      {"Swap всего"}
                    </div>
                    <div className="text-sm font-semibold">
                      {formatBytes(info.memory.total_swap_bytes)}
                    </div>
                  </div>
                  <div className="rounded-lg border border-border/60 bg-card/40 p-3">
                    <div className="text-xs text-muted-foreground">
                      {
                        "Swap используется"
                      }
                    </div>
                    <div className="text-sm font-semibold">
                      {formatBytes(info.memory.used_swap_bytes)}
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          <div className="space-y-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="text-base font-semibold">
                {"Мониторы"}
              </div>
              <div className="text-xs text-muted-foreground">
                {monitorItems.length > 0
                  ? `Найдено ${monitorItems.length}`
                  : "Не найдено"}
              </div>
            </div>
            <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
              {monitorItems.length > 0 ? (
                monitorItems.map((monitor) => (
                  <Card key={monitor.key} className="bg-card/60">
                    <CardHeader className="space-y-1">
                      <CardTitle className="text-base flex items-center gap-2">
                        <Monitor className="h-4 w-4 text-muted-foreground" />
                        {monitor.name}
                      </CardTitle>
                      <CardDescription>
                        {monitor.isPrimary
                          ? "Основной экран"
                          : "Дополнительный"}
                      </CardDescription>
                    </CardHeader>
                    <CardContent className="space-y-2">
                      <InfoRow
                        label={
                          "Разрешение"
                        }
                        value={monitor.resolution}
                      />
                      <InfoRow
                        label={"Частота"}
                        value={monitor.refresh || "—"}
                      />
                    </CardContent>
                  </Card>
                ))
              ) : (
                <Card className="bg-card/60">
                  <CardContent className="py-4 text-sm text-muted-foreground">
                    {
                      "Мониторы не найдены"
                    }
                  </CardContent>
                </Card>
              )}
            </div>
          </div>

          <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
            <Card className="bg-card/60">
              <CardHeader className="space-y-1">
                <CardTitle className="text-base flex items-center gap-2">
                  <Monitor className="h-4 w-4 text-muted-foreground" />
                  {"Система"}
                </CardTitle>
                <CardDescription>
                  {
                    "ОС и время работы"
                  }
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3">
                <InfoRow
                  label={"ОС"}
                  value={
                    [info.os_name, info.os_version].filter(Boolean).join(" ") ||
                    "—"
                  }
                />
                <InfoRow
                  label={"Ядро"}
                  value={info.kernel_version || "—"}
                />
                <InfoRow
                  label={
                    "Время работы"
                  }
                  value={formatUptime(info.uptime_seconds)}
                />
              </CardContent>
            </Card>
          </div>

          <div className="space-y-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="text-base font-semibold">
                {"Диски"}
              </div>
              <div className="text-xs text-muted-foreground">
                {diskCards.length > 0
                  ? `Найдено ${diskCards.length}`
                  : "Не найдено"}
              </div>
            </div>
            <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
              {diskCards.length > 0 ? (
                diskCards.map((disk) => {
                  const testState = diskTests[disk.mountPoint];
                  return (
                    <Card key={disk.key} className="bg-card/60">
                      <CardHeader className="space-y-1">
                        <CardTitle className="text-base flex items-center gap-2">
                          <HardDrive className="h-4 w-4 text-muted-foreground" />
                          {disk.model}
                        </CardTitle>
                        <CardDescription>
                          {`${disk.mountPoint} • ${disk.typeLabel}`}
                        </CardDescription>
                      </CardHeader>
                      <CardContent className="space-y-3">
                        <InfoRow
                          label={
                            "Использовано"
                          }
                          value={`${disk.used} / ${disk.total}`}
                        />
                        <Progress value={disk.percent} />
                        <InfoRow
                          label={
                            "Свободно"
                          }
                          value={disk.free}
                        />
                        <div className="flex flex-wrap items-center gap-2">
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleDiskTest(disk.mountPoint)}
                            disabled={testState?.loading || disk.isRemovable}
                          >
                            {testState?.loading ? (
                              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                            ) : null}
                            {
                              "Тест скорости"
                            }
                          </Button>
                          {disk.isRemovable ? (
                            <span className="text-xs text-muted-foreground">
                              {
                                "Съемный носитель"
                              }
                            </span>
                          ) : null}
                        </div>
                        {testState?.result ? (
                          <div className="rounded-lg border border-border/60 bg-card/40 p-3 text-sm">
                            <div className="flex items-center justify-between">
                              <span className="text-muted-foreground">
                                {"Запись"}
                              </span>
                              <span className="font-medium">
                                {formatSpeed(testState.result.write_mbps)}
                              </span>
                            </div>
                            <div className="flex items-center justify-between">
                              <span className="text-muted-foreground">
                                {"Чтение"}
                              </span>
                              <span className="font-medium">
                                {formatSpeed(testState.result.read_mbps)}
                              </span>
                            </div>
                          </div>
                        ) : null}
                        {testState?.error ? (
                          <div className="text-xs text-destructive">
                            {testState.error}
                          </div>
                        ) : null}
                      </CardContent>
                    </Card>
                  );
                })
              ) : (
                <Card className="bg-card/60">
                  <CardContent className="py-4 text-sm text-muted-foreground">
                    {
                      "Диски не найдены"
                    }
                  </CardContent>
                </Card>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
