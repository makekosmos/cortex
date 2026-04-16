import { type AchievementRow, DEFAULT_ACHIEVEMENTS, mapAchievementRow } from "../helpers/achievements";
import { execute, queryAll, runInTransaction } from "../helpers/db";
import type { Achievement, DbLike } from "../helpers/shared";

export interface AchievementsServiceDeps {
  db: DbLike;
}

export interface AchievementsService {
  getAllAchievements(unlockedOnly: boolean): Promise<Achievement[]>;
  seedDefaultAchievements(): Promise<Achievement[]>;
  recordAchievementEvent(eventTrigger: string, eventContext?: string | null): Promise<Achievement[]>;
}

async function seedDefaults(db: DbLike): Promise<void> {
  const createdAt = new Date().toISOString();

  await runInTransaction(db, async (tx) => {
    for (const seed of DEFAULT_ACHIEVEMENTS) {
      await execute(
        tx,
        `INSERT OR IGNORE INTO achievements
          (id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, 0, NULL, ?6)`,
        [seed.id, seed.title, seed.description, seed.event_trigger, seed.target, createdAt],
      );
    }
  });
}

async function fetchAchievements(db: DbLike, unlockedOnly: boolean): Promise<Achievement[]> {
  const rows = unlockedOnly
    ? await queryAll<AchievementRow>(
        db,
        `SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
         FROM achievements
         WHERE unlocked = 1
         ORDER BY COALESCE(unlocked_at, created_at) DESC, title ASC`,
      )
    : await queryAll<AchievementRow>(
        db,
        `SELECT id, title, description, event_trigger, progress, target, unlocked, unlocked_at, created_at
         FROM achievements
         ORDER BY unlocked DESC, COALESCE(unlocked_at, created_at) DESC, title ASC`,
      );

  return rows.map(mapAchievementRow);
}

export function createAchievementsService(deps: AchievementsServiceDeps): AchievementsService {
  return {
    async getAllAchievements(unlockedOnly: boolean): Promise<Achievement[]> {
      await seedDefaults(deps.db);
      return await fetchAchievements(deps.db, unlockedOnly);
    },

    async seedDefaultAchievements(): Promise<Achievement[]> {
      await seedDefaults(deps.db);
      return await fetchAchievements(deps.db, false);
    },

    async recordAchievementEvent(
      eventTrigger: string,
      eventContext?: string | null,
    ): Promise<Achievement[]> {
      const trigger = eventTrigger.trim();
      void eventContext;
      await seedDefaults(deps.db);

      if (trigger.length === 0) {
        return await fetchAchievements(deps.db, false);
      }

      await runInTransaction(deps.db, async (tx) => {
        const unlockedAt = new Date().toISOString();
        await execute(
          tx,
          `UPDATE achievements
           SET progress = CASE WHEN progress < target THEN progress + 1 ELSE progress END,
               unlocked = CASE WHEN progress + 1 >= target THEN 1 ELSE unlocked END,
               unlocked_at = CASE
                 WHEN unlocked = 0 AND progress + 1 >= target THEN ?1
                 ELSE unlocked_at
               END
           WHERE event_trigger = ?2`,
          [unlockedAt, trigger],
        );
      });

      return await fetchAchievements(deps.db, false);
    },
  };
}
