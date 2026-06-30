import { watch } from "vue";

import { SYSTEM_TYPE_COLLECTION_ID } from "@/lib/systemTypes";
import { migrateEntriesToTiptapJson } from "./migration";

interface AutoTipTapMigrationStore {
  vaultPath: string | null;
  isInitializing: boolean;
  isHydratingVault: boolean;
  currentEntry: Entry | null;
  isCurrentEntryDirty: boolean;
  handleSave(entry: Entry): Promise<SaveEntryResult | null>;
  updateEntryDraft(entry: Entry): void;
  refreshData(): Promise<void>;
}

export function useAutoTipTapMigration(eden: AutoTipTapMigrationStore): void {
  let running = false;
  let migratedVaultPath: string | null = null;

  async function run(): Promise<void> {
    const vaultPath = eden.vaultPath;
    if (
      running ||
      !vaultPath ||
      migratedVaultPath === vaultPath ||
      eden.isInitializing ||
      eden.isHydratingVault ||
      !window.api?.listAllEntries
    ) {
      return;
    }

    running = true;

    try {
      const entries = (await window.api.listAllEntries()).filter(
        (entry) => entry.deleted_at === null && entry.type_id !== SYSTEM_TYPE_COLLECTION_ID,
      );
      const originalContentById = new Map(entries.map((entry) => [entry.id, entry.content_json]));
      const saveMigratedEntry = async (entry: Entry): Promise<SaveEntryResult | null> => {
        if (
          eden.currentEntry?.id === entry.id &&
          (eden.isCurrentEntryDirty ||
            eden.currentEntry.content_json !== originalContentById.get(entry.id))
        ) {
          return {
            ok: false,
            reason: "stale_entry",
            message: "Запись изменяется локально",
          };
        }

        const latest = await window.api.loadEntry(entry.id);
        if (!latest || latest.content_json !== originalContentById.get(entry.id)) {
          return {
            ok: false,
            reason: "stale_entry",
            message: "Запись изменилась во время миграции",
          };
        }

        return eden.handleSave(entry);
      };
      const updateMigratedDraft = (entry: Entry): void => {
        if (eden.currentEntry?.id === entry.id && eden.isCurrentEntryDirty) return;
        eden.updateEntryDraft(entry);
      };
      const result = await migrateEntriesToTiptapJson(
        entries,
        saveMigratedEntry,
        updateMigratedDraft,
      );

      migratedVaultPath = vaultPath;

      if (result.converted > 0 || result.failed > 0) {
        await eden.refreshData();
      }

      if (result.failed > 0) {
        console.warn("[eden] TipTap auto migration finished with failures:", result.failures);
      }
    } catch (error) {
      console.warn("[eden] TipTap auto migration failed:", error);
    } finally {
      running = false;
    }
  }

  watch(
    () => [eden.vaultPath, eden.isInitializing, eden.isHydratingVault] as const,
    () => {
      void run();
    },
    { immediate: true, flush: "post" },
  );
}
