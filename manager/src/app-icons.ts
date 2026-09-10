import agendaIcon from "../../desktop/build/app-icons/agenda.png";
import arcadiaIcon from "../../desktop/build/app-icons/arcadia.png";
import dictationIcon from "../../desktop/build/app-icons/dictation.png";
import kosmosIcon from "../../desktop/build/app-icons/kosmos.png";
import memoriaIcon from "../../desktop/build/app-icons/memoria.png";
import ordoIcon from "../../desktop/build/app-icons/ordo.png";
import bigFrontendIcon from "../../desktop/src/integrations/assets/bigfrontend.svg";
import codewarsIcon from "../../desktop/src/integrations/assets/codewars.svg";
import greatFrontendIcon from "../../desktop/src/integrations/assets/greatfrontend.svg";
import hevyIcon from "../../desktop/src/integrations/assets/hevy.svg";
import huaweiHealthIcon from "../../desktop/src/integrations/assets/huawei-health.svg";
import leetcodeIcon from "../../desktop/src/integrations/assets/leetcode.svg";
import togglTrackIcon from "../../desktop/src/integrations/assets/toggl-track.svg";

const appIcons = new Map([
  ["com.kosmos.agenda", agendaIcon],
  ["com.kosmos.arcadia", arcadiaIcon],
  ["com.kosmos.dictation", dictationIcon],
  ["com.kosmos.memoria", memoriaIcon],
  ["com.kosmos.focus", ordoIcon],
  ["com.kosmos.shell", kosmosIcon],
  ["bigfrontend", bigFrontendIcon],
  ["com.kosmos.bigfrontend", bigFrontendIcon],
  ["greatfrontend", greatFrontendIcon],
  ["com.kosmos.greatfrontend", greatFrontendIcon],
  ["hevy", hevyIcon],
  ["com.kosmos.hevy", hevyIcon],
  ["huawei-health", huaweiHealthIcon],
  ["com.kosmos.huawei-health", huaweiHealthIcon],
  ["leetcode", leetcodeIcon],
  ["com.kosmos.leetcode", leetcodeIcon],
  ["codewars", codewarsIcon],
  ["com.kosmos.codewars", codewarsIcon],
  ["toggl", togglTrackIcon],
  ["com.kosmos.toggl", togglTrackIcon],
]);

export function appIcon(id: string, remote?: string | null, local?: string | null) {
  const bundled = appIcons.get(id);
  if (bundled) return bundled;
  const url = remote?.trim();
  if (url && /^(https:|http:|file:|data:image\/|\/|\.\/assets\/)/.test(url)) return url;
  const path = local?.trim();
  if (!path) return null;
  const normalized = path.replace(/\\/g, "/");
  const parts = normalized.split("/");
  return `${normalized.startsWith("/") ? "file://" : "file:///"}${parts
    .map((part, index) =>
      index === 0 && /^[A-Za-z]:$/.test(part) ? part : encodeURIComponent(part),
    )
    .join("/")}`;
}
