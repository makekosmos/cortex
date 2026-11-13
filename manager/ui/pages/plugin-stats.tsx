import { Text, View } from "nsh";
import { Card, FieldRow, SectionLabel } from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import type { PluginStat, PluginStats } from "../state";
import { LABEL_WIDTH } from "./plugin-fields";

/** Thousands separated by a space: `13 205`, not `13205`. */
function grouped(value: number): string {
  const digits = String(Math.abs(Math.round(value)));
  let out = "";
  for (let at = 0; at < digits.length; at += 1) {
    if (at > 0 && (digits.length - at) % 3 === 0) out += " ";
    out += digits[at];
  }
  return value < 0 ? `-${out}` : out;
}

/** Seconds as time the way a person reads it: «2 ч 5 мин», «40 сек». */
function asDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  if (hours > 0) return `${grouped(hours)} ч ${minutes} мин`;
  if (minutes > 0) return `${minutes} мин`;
  return `${total} сек`;
}

/** One number as the manifest asked for it to be printed. */
function statValue(field: PluginStat): string {
  if (field.value === null || field.value === undefined) return "—";
  const number = Number(field.value);
  if (!Number.isFinite(number)) return String(field.value);
  if (field.format === "duration") return asDuration(number);
  return grouped(number);
}

/** The numbers a plugin reports, resolved by the Engine: a page that names no
 * operation can still show any plugin's statistics. */
export function StatsBlock(props: { stats: PluginStats; colors: ApplicationTheme }) {
  const { colors, stats } = props;
  if (!stats.fields.length) return null;
  return (
    <View>
      <SectionLabel colors={colors}>{stats.title || "Статистика"}</SectionLabel>
      <Card colors={colors}>
        {stats.fields.map((field, index) => (
          <FieldRow
            key={field.key}
            colors={colors}
            label={field.label}
            labelWidth={LABEL_WIDTH}
            last={index === stats.fields.length - 1}
          >
            <Text style={{ fontSize: 15, color: colors.foreground }}>{statValue(field)}</Text>
          </FieldRow>
        ))}
      </Card>
    </View>
  );
}
