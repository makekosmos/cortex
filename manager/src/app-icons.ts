import agendaIcon from "../../desktop/build/app-icons/agenda.png";
import arcadiaIcon from "../../desktop/build/app-icons/arcadia.png";
import dictationIcon from "../../desktop/build/app-icons/dictation.png";
import kosmosIcon from "../../desktop/build/app-icons/kosmos.png";
import memoriaIcon from "../../desktop/build/app-icons/memoria.png";
import ordoIcon from "../../desktop/build/app-icons/ordo.png";

const appIcons = new Map([
  ["com.kosmos.agenda", agendaIcon],
  ["com.kosmos.arcadia", arcadiaIcon],
  ["com.kosmos.dictation", dictationIcon],
  ["com.kosmos.memoria", memoriaIcon],
  ["com.kosmos.focus", ordoIcon],
  ["com.kosmos.shell", kosmosIcon],
]);

export function appIcon(id: string, remote?: string | null, local?: string | null) {
  const bundled = appIcons.get(id);
  if (bundled) return bundled;
  const url = remote?.trim();
  if (url && /^(https:|http:|file:|data:image\/|\/|\.\/assets\/)/.test(url)) return url;
  const path = local?.trim();
  return path ? `file:///${path.replace(/\\/g, "/")}` : null;
}
