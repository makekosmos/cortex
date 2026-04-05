import { useState, useCallback, useMemo } from "react";

import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  TouchableOpacity,
  Alert,
  TextInput,
  ActivityIndicator,
} from "react-native";

import { router, useFocusEffect } from "expo-router";

import { Ionicons } from "@expo/vector-icons";

import { useThemeColor } from "@/lib/useThemeColor";

import { useSettingsStore } from "@/lib/stores/settings-store";

import { useSyncStore } from "@/lib/sync/sync-store";

import {
  getStats,
  getDailyStats,
  exportWorkoutsCSV,
  getWorkouts,
} from "@/lib/database";

import * as Sharing from "expo-sharing";

import * as FileSystem from "expo-file-system";

export default function ProfileTab() {
  const colors = useThemeColor();

  const { units, setUnits, restTimerSeconds, setRestTimer } =
    useSettingsStore();

  const [stats, setStats] = useState({
    totalWorkouts: 0,
    totalVolume: 0,
    currentStreak: 0,
  });

  const [workouts, setWorkouts] = useState<any[]>([]);

  const [heatmapDays, setHeatmapDays] = useState<Set<string>>(new Set());

  const [showSettings, setShowSettings] = useState(false);

  // Sync state

  const {
    isPaired,
    isConnected,
    isSyncing,
    isPairing,
    pairingError,
    lastSyncAt,

    pair,
    unpair,
    connect,
    disconnect,
  } = useSyncStore();

  const [showPairing, setShowPairing] = useState(false);

  const [pairingServer, setPairingServer] = useState("");

  const [pairingCode, setPairingCode] = useState("");

  useFocusEffect(
    useCallback(() => {
      loadData();
    }, []),
  );

  async function loadData() {
    const [s, w, daily] = await Promise.all([
      getStats(),

      getWorkouts(),

      getDailyStats(35),
    ]);

    setStats(s);

    setWorkouts(w);

    setHeatmapDays(new Set(daily.map((d) => d.date)));
  }

  // Generate heatmap grid for current month (5 weeks)

  const heatmapGrid = useMemo(() => {
    const today = new Date();

    const weeks: { date: string; active: boolean; future: boolean }[][] = [];

    // Start from 4 weeks ago Monday

    const start = new Date(today);

    start.setDate(start.getDate() - 34);

    // Align to Monday

    const dayOfWeek = start.getDay();

    start.setDate(start.getDate() - ((dayOfWeek + 6) % 7));

    for (let w = 0; w < 5; w++) {
      const week: (typeof weeks)[0] = [];

      for (let d = 0; d < 7; d++) {
        const current = new Date(start);

        current.setDate(start.getDate() + w * 7 + d);

        const dateStr = current.toISOString().split("T")[0];

        week.push({
          date: dateStr,

          active: heatmapDays.has(dateStr),

          future: current > today,
        });
      }

      weeks.push(week);
    }

    return weeks;
  }, [heatmapDays]);

  async function handleExportCSV() {
    try {
      const csv = await exportWorkoutsCSV();

      const path = FileSystem.cacheDirectory + "workouts_export.csv";

      await FileSystem.writeAsStringAsync(path, csv);

      await Sharing.shareAsync(path, { mimeType: "text/csv" });
    } catch (e) {
      Alert.alert("Ошибка экспорта", String(e));
    }
  }

  function formatVolume(kg: number) {
    if (units === "lbs") kg = kg * 2.205;

    if (kg >= 1000000) return `${(kg / 1000000).toFixed(1)}M`;

    if (kg >= 1000) return `${(kg / 1000).toFixed(1)}K`;

    return kg.toFixed(0);
  }

  function formatDuration(seconds: number | null) {
    if (!seconds) return "--";

    const h = Math.floor(seconds / 3600);

    const m = Math.floor((seconds % 3600) / 60);

    if (h > 0) return `${h}ч ${m}м`;

    return `${m}м`;
  }

  function formatDate(iso: string) {
    return new Date(iso).toLocaleDateString("ru-RU", {
      weekday: "short",
      day: "numeric",
      month: "short",
    });
  }

  return (
    <ScrollView
      style={[styles.container, { backgroundColor: colors.background }]}
    >
      {/* Top row: greeting + settings icon */}
      <View style={styles.topRow}>
        <View>
          <Text style={[styles.greeting, { color: colors.textSecondary }]}>
            {new Date().toLocaleDateString("ru-RU", {
              weekday: "long",
              day: "numeric",
              month: "long",
            })}
          </Text>
          <Text style={[styles.title, { color: colors.text }]}>Профиль</Text>
        </View>
        <TouchableOpacity
          onPress={() => setShowSettings(true)}
          style={[styles.settingsBtn, { backgroundColor: colors.surface }]}
        >
          <Ionicons
            name="settings-outline"
            size={20}
            color={colors.textSecondary}
          />
        </TouchableOpacity>
      </View>

      {/* ===== STRENGTH CARD ===== */}
      <TouchableOpacity
        activeOpacity={0.8}
        onPress={() => router.push("/stats/strength")}
        style={styles.strengthCard}
      >
        <View style={styles.strengthHeader}>
          <View style={styles.strengthBadge}>
            <Ionicons name="barbell" size={12} color="#EF4444" />
            <Text style={styles.strengthBadgeText}>СИЛОВЫЕ</Text>
          </View>
          <Ionicons name="chevron-forward" size={18} color="#EF444480" />
        </View>

        {/* Quick stats */}
        <View style={styles.strengthStats}>
          <View style={styles.strengthStatItem}>
            <Text style={styles.strengthStatNumber}>{stats.totalWorkouts}</Text>
            <Text style={styles.strengthStatLabel}>тренировок</Text>
          </View>
          <View style={styles.strengthStatItem}>
            <Text style={styles.strengthStatNumber}>{stats.currentStreak}</Text>
            <Text style={styles.strengthStatLabel}>дней подряд</Text>
          </View>
          <View style={styles.strengthStatItem}>
            <Text style={styles.strengthStatNumber}>
              {formatVolume(stats.totalVolume)}
            </Text>
            <Text style={styles.strengthStatLabel}>{units} всего</Text>
          </View>
        </View>

        {/* Heatmap */}
        <View style={styles.heatmap}>
          {["Пн", "", "Ср", "", "Пт", "", "Вс"].map((label, i) => (
            <Text key={i} style={styles.heatmapDayLabel}>
              {label}
            </Text>
          ))}
          {heatmapGrid.map((week, wi) => (
            <View key={wi} style={styles.heatmapWeek}>
              {week.map((day, di) => (
                <View
                  key={di}
                  style={[
                    styles.heatmapCell,

                    day.future
                      ? {
                          backgroundColor: "transparent",
                          borderWidth: 1,
                          borderColor: "#2a1515",
                        }
                      : day.active
                        ? { backgroundColor: "#EF4444" }
                        : { backgroundColor: "#1f1010" },
                  ]}
                />
              ))}
            </View>
          ))}
        </View>
      </TouchableOpacity>

      {/* ===== HISTORY CARD ===== */}
      <View style={styles.historyCard}>
        <View style={styles.historyHeader}>
          <View style={styles.historyBadge}>
            <Ionicons name="time" size={12} color="#22C55E" />
            <Text style={styles.historyBadgeText}>ПОСЛЕДНИЕ</Text>
          </View>
        </View>

        {workouts.length === 0 ? (
          <Text style={[styles.emptyText, { color: colors.textTertiary }]}>
            Пока нет тренировок
          </Text>
        ) : (
          workouts.slice(0, 5).map((item) => (
            <TouchableOpacity
              key={item.id}
              style={styles.historyRow}
              onPress={() => router.push(`/workout/${item.id}`)}
              activeOpacity={0.7}
            >
              <View style={styles.historyDot} />
              <View style={{ flex: 1 }}>
                <Text style={[styles.historyTitle, { color: colors.text }]}>
                  {item.title}
                </Text>
                <Text
                  style={[styles.historyMeta, { color: colors.textTertiary }]}
                >
                  {formatDate(item.started_at)} ·{" "}
                  {formatDuration(item.duration_seconds)}
                </Text>
              </View>
              <Ionicons
                name="chevron-forward"
                size={16}
                color={colors.textTertiary}
              />
            </TouchableOpacity>
          ))
        )}
      </View>

      {/* ===== EXPORT CARD ===== */}
      <TouchableOpacity
        style={styles.exportCard}
        onPress={handleExportCSV}
        activeOpacity={0.8}
      >
        <Ionicons name="download-outline" size={18} color="#3B82F6" />
        <Text style={styles.exportText}>Экспорт данных (CSV)</Text>
        <Ionicons name="chevron-forward" size={16} color="#3B82F620" />
      </TouchableOpacity>

      {/* ===== SYNC CARD ===== */}
      <View style={styles.syncCard}>
        <View style={styles.syncHeader}>
          <View style={styles.syncBadge}>
            <Ionicons name="sync" size={12} color="#8B5CF6" />
            <Text style={styles.syncBadgeText}>ARK SYNC</Text>
          </View>
          {isPaired && (
            <View
              style={[
                styles.syncDot,
                { backgroundColor: isConnected ? "#22C55E" : "#F59E0B" },
              ]}
            />
          )}
        </View>

        {isPaired ? (
          <>
            <Text style={[styles.syncStatus, { color: colors.text }]}>
              {isConnected
                ? "Подключено"
                : isSyncing
                  ? "Подключение..."
                  : "Отключено"}
            </Text>
            {lastSyncAt && (
              <Text style={[styles.syncMeta, { color: colors.textTertiary }]}>
                Последняя синхронизация:{" "}
                {new Date(lastSyncAt).toLocaleString("ru-RU")}
              </Text>
            )}
            <View style={styles.syncActions}>
              <TouchableOpacity
                style={[
                  styles.syncActionBtn,
                  { backgroundColor: colors.surface },
                ]}
                onPress={() => (isConnected ? disconnect() : connect())}
              >
                <Text style={[styles.syncActionText, { color: colors.text }]}>
                  {isConnected ? "Отключить" : "Подключить"}
                </Text>
              </TouchableOpacity>
              <TouchableOpacity
                style={[styles.syncActionBtn, { backgroundColor: "#EF444420" }]}
                onPress={() =>
                  Alert.alert(
                    "Отвязать устройство?",
                    "Данные синхронизации будут удалены.",
                    [
                      { text: "Отмена", style: "cancel" },

                      {
                        text: "Отвязать",
                        style: "destructive",
                        onPress: unpair,
                      },
                    ],
                  )
                }
              >
                <Text style={[styles.syncActionText, { color: "#EF4444" }]}>
                  Отвязать
                </Text>
              </TouchableOpacity>
            </View>
          </>
        ) : (
          <>
            <Text style={[styles.syncStatus, { color: colors.textSecondary }]}>
              Не привязано к серверу Ark
            </Text>

            {!showPairing ? (
              <TouchableOpacity
                style={[styles.syncPairBtn, { backgroundColor: "#8B5CF620" }]}
                onPress={() => setShowPairing(true)}
              >
                <Ionicons name="link" size={16} color="#8B5CF6" />
                <Text style={styles.syncPairBtnText}>Привязать устройство</Text>
              </TouchableOpacity>
            ) : (
              <View style={styles.pairingForm}>
                <TextInput
                  style={[
                    styles.pairingInput,
                    { color: colors.text, borderColor: colors.border },
                  ]}
                  placeholder="https://ark.example.com"
                  placeholderTextColor={colors.textTertiary}
                  value={pairingServer}
                  onChangeText={setPairingServer}
                  autoCapitalize="none"
                  autoCorrect={false}
                  keyboardType="url"
                />
                <TextInput
                  style={[
                    styles.pairingInput,
                    { color: colors.text, borderColor: colors.border },
                  ]}
                  placeholder="ark-XXXX"
                  placeholderTextColor={colors.textTertiary}
                  value={pairingCode}
                  onChangeText={setPairingCode}
                  autoCapitalize="none"
                  autoCorrect={false}
                />
                {pairingError && (
                  <Text style={styles.pairingError}>{pairingError}</Text>
                )}
                <View style={styles.pairingActions}>
                  <TouchableOpacity
                    style={[
                      styles.syncActionBtn,
                      { backgroundColor: colors.surface },
                    ]}
                    onPress={() => {
                      setShowPairing(false);
                      setPairingServer("");
                      setPairingCode("");
                    }}
                  >
                    <Text
                      style={[
                        styles.syncActionText,
                        { color: colors.textSecondary },
                      ]}
                    >
                      Отмена
                    </Text>
                  </TouchableOpacity>
                  <TouchableOpacity
                    style={[
                      styles.syncActionBtn,
                      { backgroundColor: "#8B5CF6" },
                    ]}
                    disabled={isPairing || !pairingServer || !pairingCode}
                    onPress={async () => {
                      try {
                        await pair(pairingServer, pairingCode);

                        setShowPairing(false);

                        setPairingServer("");

                        setPairingCode("");
                      } catch {
                        // error is shown via pairingError state
                      }
                    }}
                  >
                    {isPairing ? (
                      <ActivityIndicator size="small" color="#fff" />
                    ) : (
                      <Text style={[styles.syncActionText, { color: "#fff" }]}>
                        Привязать
                      </Text>
                    )}
                  </TouchableOpacity>
                </View>
              </View>
            )}
          </>
        )}
      </View>

      {/* ===== SETTINGS MODAL ===== */}
      {showSettings && (
        <View style={styles.settingsOverlay}>
          <View
            style={[styles.settingsCard, { backgroundColor: colors.surface }]}
          >
            <View style={styles.settingsHeader}>
              <Text style={[styles.settingsTitle, { color: colors.text }]}>
                Настройки
              </Text>
              <TouchableOpacity onPress={() => setShowSettings(false)}>
                <Ionicons name="close" size={22} color={colors.textSecondary} />
              </TouchableOpacity>
            </View>

            <View style={styles.settingRow}>
              <Text style={[styles.settingLabel, { color: colors.text }]}>
                Единицы
              </Text>
              <View style={styles.toggleRow}>
                {(["kg", "lbs"] as const).map((u) => (
                  <TouchableOpacity
                    key={u}
                    style={[
                      styles.toggleBtn,
                      units === u && { backgroundColor: colors.accent },
                    ]}
                    onPress={() => setUnits(u)}
                  >
                    <Text
                      style={[
                        styles.toggleText,
                        { color: units === u ? "#fff" : colors.textSecondary },
                      ]}
                    >
                      {u === "kg" ? "кг" : "lbs"}
                    </Text>
                  </TouchableOpacity>
                ))}
              </View>
            </View>

            <View style={styles.settingRow}>
              <Text style={[styles.settingLabel, { color: colors.text }]}>
                Отдых по умолчанию
              </Text>
              <View style={styles.toggleRow}>
                {[60, 90, 120, 180].map((s) => (
                  <TouchableOpacity
                    key={s}
                    style={[
                      styles.toggleBtn,
                      restTimerSeconds === s && {
                        backgroundColor: colors.accent,
                      },
                    ]}
                    onPress={() => setRestTimer(s)}
                  >
                    <Text
                      style={[
                        styles.toggleText,
                        {
                          color:
                            restTimerSeconds === s
                              ? "#fff"
                              : colors.textSecondary,
                        },
                      ]}
                    >
                      {s}с
                    </Text>
                  </TouchableOpacity>
                ))}
              </View>
            </View>
          </View>
        </View>
      )}

      <View style={{ height: 40 }} />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1 },

  // Top

  topRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "flex-start",

    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 16,
  },

  greeting: { fontSize: 13, marginBottom: 4, textTransform: "capitalize" },

  title: { fontSize: 28, fontWeight: "800" },

  settingsBtn: {
    width: 40,
    height: 40,
    borderRadius: 20,

    justifyContent: "center",
    alignItems: "center",
  },

  // Strength card

  strengthCard: {
    marginHorizontal: 16,
    marginBottom: 14,
    borderRadius: 16,
    padding: 16,

    backgroundColor: "#160808",
    borderWidth: 1,
    borderColor: "#2a1010",
  },

  strengthHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",

    marginBottom: 14,
  },

  strengthBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,

    backgroundColor: "#EF4444" + "18",
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
  },

  strengthBadgeText: {
    color: "#EF4444",
    fontSize: 11,
    fontWeight: "800",
    letterSpacing: 1.5,
  },

  strengthStats: { flexDirection: "row", marginBottom: 16 },

  strengthStatItem: { flex: 1 },

  strengthStatNumber: { color: "#EF4444", fontSize: 24, fontWeight: "800" },

  strengthStatLabel: { color: "#EF444460", fontSize: 11, marginTop: 2 },

  // Heatmap

  heatmap: {
    flexDirection: "row",
    gap: 3,
    alignItems: "flex-start",
  },

  heatmapDayLabel: {
    width: 14,
    fontSize: 9,
    color: "#EF444440",
    textAlign: "center",

    height: 14,
    lineHeight: 14,
    marginBottom: 1,
  },

  heatmapWeek: { gap: 3 },

  heatmapCell: { width: 14, height: 14, borderRadius: 3 },

  // History card

  historyCard: {
    marginHorizontal: 16,
    marginBottom: 14,
    borderRadius: 16,
    padding: 16,

    backgroundColor: "#081210",
    borderWidth: 1,
    borderColor: "#102a1a",
  },

  historyHeader: { marginBottom: 10 },

  historyBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,

    backgroundColor: "#22C55E" + "18",
    paddingHorizontal: 10,
    paddingVertical: 4,

    borderRadius: 6,
    alignSelf: "flex-start",
  },

  historyBadgeText: {
    color: "#22C55E",
    fontSize: 11,
    fontWeight: "800",
    letterSpacing: 1.5,
  },

  emptyText: { fontSize: 14, textAlign: "center", paddingVertical: 16 },

  historyRow: {
    flexDirection: "row",
    alignItems: "center",
    paddingVertical: 10,

    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: "#102a1a",
    gap: 10,
  },

  historyDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: "#22C55E",
  },

  historyTitle: { fontSize: 15, fontWeight: "600" },

  historyMeta: { fontSize: 12, marginTop: 2 },

  // Export

  exportCard: {
    marginHorizontal: 16,
    marginBottom: 14,
    borderRadius: 16,
    padding: 16,

    backgroundColor: "#080d1a",
    borderWidth: 1,
    borderColor: "#101a3a",

    flexDirection: "row",
    alignItems: "center",
    gap: 10,
  },

  exportText: { color: "#3B82F6", fontSize: 14, fontWeight: "600", flex: 1 },

  // Settings overlay

  settingsOverlay: {
    position: "absolute",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,

    backgroundColor: "#00000090",
    justifyContent: "center",
    padding: 24,
  },

  settingsCard: { borderRadius: 16, padding: 20 },

  settingsHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",

    marginBottom: 20,
  },

  settingsTitle: { fontSize: 18, fontWeight: "700" },

  settingRow: {
    flexDirection: "row",
    justifyContent: "space-between",

    alignItems: "center",
    marginBottom: 16,
  },

  settingLabel: { fontSize: 14 },

  toggleRow: { flexDirection: "row", gap: 4 },

  toggleBtn: { paddingHorizontal: 12, paddingVertical: 6, borderRadius: 6 },

  toggleText: { fontSize: 13, fontWeight: "500" },

  // Sync card

  syncCard: {
    marginHorizontal: 16,
    marginBottom: 14,
    borderRadius: 16,
    padding: 16,

    backgroundColor: "#0d0818",
    borderWidth: 1,
    borderColor: "#1a103a",
  },

  syncHeader: {
    flexDirection: "row",
    justifyContent: "space-between",
    alignItems: "center",

    marginBottom: 10,
  },

  syncBadge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,

    backgroundColor: "#8B5CF618",
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 6,
  },

  syncBadgeText: {
    color: "#8B5CF6",
    fontSize: 11,
    fontWeight: "800",
    letterSpacing: 1.5,
  },

  syncDot: { width: 8, height: 8, borderRadius: 4 },

  syncStatus: { fontSize: 15, fontWeight: "600", marginBottom: 4 },

  syncMeta: { fontSize: 12, marginBottom: 12 },

  syncActions: { flexDirection: "row", gap: 8, marginTop: 8 },

  syncActionBtn: {
    flex: 1,
    paddingVertical: 10,
    borderRadius: 8,

    justifyContent: "center",
    alignItems: "center",
  },

  syncActionText: { fontSize: 13, fontWeight: "600" },

  syncPairBtn: {
    flexDirection: "row",
    alignItems: "center",
    gap: 8,

    paddingVertical: 10,
    paddingHorizontal: 14,
    borderRadius: 8,
    marginTop: 8,

    alignSelf: "flex-start",
  },

  syncPairBtnText: { color: "#8B5CF6", fontSize: 14, fontWeight: "600" },

  // Pairing form

  pairingForm: { marginTop: 10, gap: 10 },

  pairingInput: {
    borderWidth: 1,
    borderRadius: 8,
    paddingHorizontal: 12,
    paddingVertical: 10,

    fontSize: 14,
  },

  pairingError: { color: "#EF4444", fontSize: 12 },

  pairingActions: { flexDirection: "row", gap: 8 },
});
