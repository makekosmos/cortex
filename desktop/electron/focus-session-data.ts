import type { ArkObjectLike, DelphiTask, FocusBlocklist } from "./focus-session-types";
import { isRecord, isString, type JsonRecord } from "../src/shared/runtimeGuards";

type Invoke = <T = unknown>(operation: string, params?: JsonRecord) => Promise<T>;

const EMPTY_PROPS: JsonRecord = Object.freeze({});

function toProps(record: ArkObjectLike): JsonRecord {
  const props = record.propsJson ?? record.props_json;
  return isRecord(props) ? props : EMPTY_PROPS;
}

interface FocusSessionDataApi {
  resolveCategoryDomains: (categoryIds: string[]) => Promise<string[]>;
  listTasks: () => Promise<DelphiTask[]>;
  listBlocklists: () => Promise<FocusBlocklist[]>;
}

export function createFocusSessionDataApi(invoke: Invoke): FocusSessionDataApi {
  async function resolveBlocklistDomains(blocklistId: string): Promise<string[]> {
    try {
      const resolved = await invoke<{ domains?: string[] }>("focus.resolve_blocklist_domains", {
        id: blocklistId,
      });
      return Array.isArray(resolved?.domains) ? resolved.domains : [];
    } catch {
      const resp = await invoke<{ blocklists?: FocusBlocklist[] }>("focus.list_blocklists");
      const found = resp.blocklists?.find((b) => b.id === blocklistId);
      return Array.isArray(found?.domains) ? found.domains : [];
    }
  }

  return {
    async resolveCategoryDomains(categoryIds: string[]): Promise<string[]> {
      const merged = new Set<string>();
      for (const id of categoryIds) {
        const domains = await resolveBlocklistDomains(id);
        for (const domain of domains) merged.add(domain);
      }
      return Array.from(merged).sort((a, b) => a.localeCompare(b));
    },

    async listTasks(): Promise<DelphiTask[]> {
      const list = await invoke<ArkObjectLike[]>("list_objects_by_type", { type_id: "task_obj" });
      return (Array.isArray(list) ? list : [])
        .filter((o) => !o.deletedAt && !o.deleted_at)
        .map((o) => {
          const props = toProps(o);
          const status = isString(props.status) ? props.status : null;
          const isCompleted = props.isCompleted === true || props.is_completed === true;
          const isCancelled = props.isCancelled === true || props.is_cancelled === true;
          const isTrashed = props.isTrashed === true || props.is_trashed === true;
          return {
            id: o.id,
            title: o.title ?? "",
            status,
            isCompleted,
            isCancelled,
            isTrashed,
          };
        })
        .filter((t) => {
          if (t.title.length === 0) return false;
          if (t.isCompleted || t.isCancelled || t.isTrashed) return false;
          return t.status !== "done" && t.status !== "canceled" && t.status !== "cancelled";
        })
        .map(({ id, title, status }) => ({ id, title, status }));
    },

    async listBlocklists(): Promise<FocusBlocklist[]> {
      const resp = await invoke<{ blocklists?: FocusBlocklist[] }>("focus.list_blocklists");
      return Array.isArray(resp.blocklists) ? resp.blocklists : [];
    },
  };
}
