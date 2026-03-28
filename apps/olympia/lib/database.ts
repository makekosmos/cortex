import * as SQLite from 'expo-sqlite';

const DB_NAME = 'workout-tracker.db';

let db: SQLite.SQLiteDatabase | null = null;

export async function getDatabase(): Promise<SQLite.SQLiteDatabase> {
  if (db) return db;
  db = await SQLite.openDatabaseAsync(DB_NAME);
  await db.execAsync('PRAGMA journal_mode = WAL;');
  await db.execAsync('PRAGMA foreign_keys = ON;');
  return db;
}

export async function initializeDatabase(): Promise<void> {
  const database = await getDatabase();

  await database.execAsync(`
    CREATE TABLE IF NOT EXISTS exercises (
      id TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      force TEXT,
      level TEXT,
      mechanic TEXT,
      equipment TEXT,
      primary_muscles TEXT,
      secondary_muscles TEXT,
      instructions TEXT,
      images TEXT,
      category TEXT,
      is_custom INTEGER DEFAULT 0
    );

    CREATE TABLE IF NOT EXISTS workouts (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      started_at TEXT NOT NULL,
      finished_at TEXT,
      notes TEXT,
      duration_seconds INTEGER
    );

    CREATE TABLE IF NOT EXISTS workout_exercises (
      id TEXT PRIMARY KEY,
      workout_id TEXT NOT NULL,
      exercise_id TEXT NOT NULL,
      sort_order INTEGER NOT NULL,
      notes TEXT,
      FOREIGN KEY (workout_id) REFERENCES workouts(id) ON DELETE CASCADE,
      FOREIGN KEY (exercise_id) REFERENCES exercises(id)
    );

    CREATE TABLE IF NOT EXISTS workout_sets (
      id TEXT PRIMARY KEY,
      workout_exercise_id TEXT NOT NULL,
      set_index INTEGER NOT NULL,
      set_type TEXT DEFAULT 'normal',
      weight_kg REAL,
      reps INTEGER,
      duration_seconds INTEGER,
      distance_meters REAL,
      completed INTEGER DEFAULT 0,
      rpe REAL,
      FOREIGN KEY (workout_exercise_id) REFERENCES workout_exercises(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS routines (
      id TEXT PRIMARY KEY,
      title TEXT NOT NULL,
      created_at TEXT NOT NULL,
      updated_at TEXT
    );

    CREATE TABLE IF NOT EXISTS routine_exercises (
      id TEXT PRIMARY KEY,
      routine_id TEXT NOT NULL,
      exercise_id TEXT NOT NULL,
      sort_order INTEGER NOT NULL,
      target_sets INTEGER,
      target_reps TEXT,
      target_weight_kg REAL,
      notes TEXT,
      FOREIGN KEY (routine_id) REFERENCES routines(id) ON DELETE CASCADE,
      FOREIGN KEY (exercise_id) REFERENCES exercises(id)
    );

    CREATE TABLE IF NOT EXISTS body_weight_log (
      id TEXT PRIMARY KEY,
      date TEXT NOT NULL,
      weight_kg REAL NOT NULL,
      notes TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_workout_exercises_workout ON workout_exercises(workout_id);
    CREATE INDEX IF NOT EXISTS idx_workout_sets_exercise ON workout_sets(workout_exercise_id);
    CREATE INDEX IF NOT EXISTS idx_routine_exercises_routine ON routine_exercises(routine_id);
    CREATE INDEX IF NOT EXISTS idx_workouts_started ON workouts(started_at DESC);
  `);
}

export async function seedExercises(): Promise<void> {
  const database = await getDatabase();

  // Check if already seeded
  const result = await database.getFirstAsync<{ count: number }>(
    'SELECT COUNT(*) as count FROM exercises WHERE is_custom = 0'
  );
  if (result && result.count > 0) return;

  const exercises = require('../assets/data/exercises.json');

  // Insert in batches
  const BATCH_SIZE = 50;
  for (let i = 0; i < exercises.length; i += BATCH_SIZE) {
    const batch = exercises.slice(i, i + BATCH_SIZE);
    const placeholders = batch.map(() => '(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)').join(',');
    const values = batch.flatMap((ex: any) => [
      ex.id,
      ex.name,
      ex.force || null,
      ex.level || null,
      ex.mechanic || null,
      ex.equipment || null,
      JSON.stringify(ex.primaryMuscles || []),
      JSON.stringify(ex.secondaryMuscles || []),
      JSON.stringify(ex.instructions || []),
      JSON.stringify(ex.images || []),
      ex.category || null,
    ]);
    await database.runAsync(
      `INSERT OR IGNORE INTO exercises (id, name, force, level, mechanic, equipment, primary_muscles, secondary_muscles, instructions, images, category, is_custom) VALUES ${placeholders}`,
      values
    );
  }
}

// ============ Exercise Queries ============

export async function getExercises(
  search?: string,
  muscle?: string,
  equipment?: string,
  level?: string
): Promise<any[]> {
  const database = await getDatabase();
  let query = 'SELECT * FROM exercises WHERE 1=1';
  const params: any[] = [];

  if (search) {
    query += ' AND name LIKE ?';
    params.push(`%${search}%`);
  }
  if (muscle) {
    query += ' AND primary_muscles LIKE ?';
    params.push(`%${muscle}%`);
  }
  if (equipment) {
    query += ' AND equipment = ?';
    params.push(equipment);
  }
  if (level) {
    query += ' AND level = ?';
    params.push(level);
  }

  query += ' ORDER BY name ASC';
  const rows = await database.getAllAsync(query, params);
  return rows.map(parseExerciseRow);
}

export async function getExerciseById(id: string): Promise<any | null> {
  const database = await getDatabase();
  const row = await database.getFirstAsync('SELECT * FROM exercises WHERE id = ?', [id]);
  return row ? parseExerciseRow(row) : null;
}

function parseExerciseRow(row: any) {
  return {
    ...row,
    primary_muscles: JSON.parse(row.primary_muscles || '[]'),
    secondary_muscles: JSON.parse(row.secondary_muscles || '[]'),
    instructions: JSON.parse(row.instructions || '[]'),
    images: JSON.parse(row.images || '[]'),
    is_custom: row.is_custom === 1,
  };
}

export async function createCustomExercise(exercise: {
  id: string;
  name: string;
  primary_muscles: string[];
  equipment: string | null;
  instructions: string[];
}): Promise<void> {
  const database = await getDatabase();
  await database.runAsync(
    `INSERT INTO exercises (id, name, force, level, mechanic, equipment, primary_muscles, secondary_muscles, instructions, images, category, is_custom)
     VALUES (?, ?, NULL, 'intermediate', NULL, ?, ?, '[]', ?, '[]', NULL, 1)`,
    [
      exercise.id,
      exercise.name,
      exercise.equipment,
      JSON.stringify(exercise.primary_muscles),
      JSON.stringify(exercise.instructions),
    ]
  );
}

// ============ Workout Queries ============

export async function saveWorkout(
  workout: { id: string; title: string; started_at: string; finished_at: string; notes: string; duration_seconds: number },
  exercises: {
    id: string;
    exercise_id: string;
    sort_order: number;
    notes: string;
    sets: {
      id: string;
      set_index: number;
      set_type: string;
      weight_kg: number | null;
      reps: number | null;
      completed: boolean;
      rpe: number | null;
    }[];
  }[]
): Promise<void> {
  const database = await getDatabase();

  await database.withTransactionAsync(async () => {
    await database.runAsync(
      'INSERT INTO workouts (id, title, started_at, finished_at, notes, duration_seconds) VALUES (?, ?, ?, ?, ?, ?)',
      [workout.id, workout.title, workout.started_at, workout.finished_at, workout.notes, workout.duration_seconds]
    );

    for (const ex of exercises) {
      await database.runAsync(
        'INSERT INTO workout_exercises (id, workout_id, exercise_id, sort_order, notes) VALUES (?, ?, ?, ?, ?)',
        [ex.id, workout.id, ex.exercise_id, ex.sort_order, ex.notes]
      );

      for (const set of ex.sets) {
        await database.runAsync(
          'INSERT INTO workout_sets (id, workout_exercise_id, set_index, set_type, weight_kg, reps, completed, rpe) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
          [set.id, ex.id, set.set_index, set.set_type, set.weight_kg, set.reps, set.completed ? 1 : 0, set.rpe]
        );
      }
    }
  });
}

export async function getWorkouts(): Promise<any[]> {
  const database = await getDatabase();
  return database.getAllAsync(
    'SELECT * FROM workouts ORDER BY started_at DESC'
  );
}

export async function getWorkoutWithDetails(workoutId: string): Promise<any | null> {
  const database = await getDatabase();
  const workout = await database.getFirstAsync('SELECT * FROM workouts WHERE id = ?', [workoutId]);
  if (!workout) return null;

  const exercises = await database.getAllAsync(
    `SELECT we.*, e.name as exercise_name, e.primary_muscles, e.images
     FROM workout_exercises we
     JOIN exercises e ON e.id = we.exercise_id
     WHERE we.workout_id = ?
     ORDER BY we.sort_order`,
    [workoutId]
  );

  for (const ex of exercises as any[]) {
    ex.primary_muscles = JSON.parse(ex.primary_muscles || '[]');
    ex.images = JSON.parse(ex.images || '[]');
    ex.sets = await database.getAllAsync(
      'SELECT * FROM workout_sets WHERE workout_exercise_id = ? ORDER BY set_index',
      [ex.id]
    );
  }

  return { ...workout, exercises };
}

export async function deleteWorkout(workoutId: string): Promise<void> {
  const database = await getDatabase();
  await database.runAsync('DELETE FROM workouts WHERE id = ?', [workoutId]);
}

export async function getLastSetsForExercise(exerciseId: string): Promise<any[]> {
  const database = await getDatabase();
  const lastWorkoutExercise = await database.getFirstAsync(
    `SELECT we.id FROM workout_exercises we
     JOIN workouts w ON w.id = we.workout_id
     WHERE we.exercise_id = ? AND w.finished_at IS NOT NULL
     ORDER BY w.started_at DESC LIMIT 1`,
    [exerciseId]
  );
  if (!lastWorkoutExercise) return [];
  return database.getAllAsync(
    'SELECT * FROM workout_sets WHERE workout_exercise_id = ? ORDER BY set_index',
    [(lastWorkoutExercise as any).id]
  );
}

export async function getExerciseHistory(exerciseId: string): Promise<any[]> {
  const database = await getDatabase();
  return database.getAllAsync(
    `SELECT w.started_at, ws.weight_kg, ws.reps, ws.set_type
     FROM workout_sets ws
     JOIN workout_exercises we ON we.id = ws.workout_exercise_id
     JOIN workouts w ON w.id = we.workout_id
     WHERE we.exercise_id = ? AND w.finished_at IS NOT NULL AND ws.completed = 1
     ORDER BY w.started_at ASC`,
    [exerciseId]
  );
}

export async function getRecentExerciseIds(): Promise<string[]> {
  const database = await getDatabase();
  const rows = await database.getAllAsync(
    `SELECT DISTINCT we.exercise_id FROM workout_exercises we
     JOIN workouts w ON w.id = we.workout_id
     ORDER BY w.started_at DESC LIMIT 20`
  );
  return (rows as any[]).map(r => r.exercise_id);
}

// ============ Routine Queries ============

export async function saveRoutine(
  routine: { id: string; title: string },
  exercises: { id: string; exercise_id: string; sort_order: number; target_sets: number; target_reps: string; target_weight_kg: number | null; notes: string }[]
): Promise<void> {
  const database = await getDatabase();
  const now = new Date().toISOString();
  await database.withTransactionAsync(async () => {
    await database.runAsync(
      'INSERT INTO routines (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)',
      [routine.id, routine.title, now, now]
    );
    for (const ex of exercises) {
      await database.runAsync(
        'INSERT INTO routine_exercises (id, routine_id, exercise_id, sort_order, target_sets, target_reps, target_weight_kg, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?)',
        [ex.id, routine.id, ex.exercise_id, ex.sort_order, ex.target_sets, ex.target_reps, ex.target_weight_kg, ex.notes]
      );
    }
  });
}

export async function getRoutines(): Promise<any[]> {
  const database = await getDatabase();
  return database.getAllAsync('SELECT * FROM routines ORDER BY updated_at DESC');
}

export async function getRoutineWithExercises(routineId: string): Promise<any | null> {
  const database = await getDatabase();
  const routine = await database.getFirstAsync('SELECT * FROM routines WHERE id = ?', [routineId]);
  if (!routine) return null;

  const exercises = await database.getAllAsync(
    `SELECT re.*, e.name as exercise_name, e.primary_muscles, e.images
     FROM routine_exercises re
     JOIN exercises e ON e.id = re.exercise_id
     WHERE re.routine_id = ?
     ORDER BY re.sort_order`,
    [routineId]
  );

  for (const ex of exercises as any[]) {
    ex.primary_muscles = JSON.parse(ex.primary_muscles || '[]');
    ex.images = JSON.parse(ex.images || '[]');
  }

  return { ...routine, exercises };
}

export async function deleteRoutine(routineId: string): Promise<void> {
  const database = await getDatabase();
  await database.runAsync('DELETE FROM routines WHERE id = ?', [routineId]);
}

// ============ Body Weight Queries ============

export async function saveBodyWeight(entry: { id: string; date: string; weight_kg: number; notes: string | null }): Promise<void> {
  const database = await getDatabase();
  await database.runAsync(
    'INSERT OR REPLACE INTO body_weight_log (id, date, weight_kg, notes) VALUES (?, ?, ?, ?)',
    [entry.id, entry.date, entry.weight_kg, entry.notes]
  );
}

export async function getBodyWeightLog(): Promise<any[]> {
  const database = await getDatabase();
  return database.getAllAsync('SELECT * FROM body_weight_log ORDER BY date DESC');
}

// ============ Stats ============

export async function getStats(): Promise<{ totalWorkouts: number; totalVolume: number; currentStreak: number }> {
  const database = await getDatabase();

  const countResult = await database.getFirstAsync<{ count: number }>(
    'SELECT COUNT(*) as count FROM workouts WHERE finished_at IS NOT NULL'
  );

  const volumeResult = await database.getFirstAsync<{ total: number }>(
    `SELECT COALESCE(SUM(ws.weight_kg * ws.reps), 0) as total
     FROM workout_sets ws
     JOIN workout_exercises we ON we.id = ws.workout_exercise_id
     JOIN workouts w ON w.id = we.workout_id
     WHERE w.finished_at IS NOT NULL AND ws.completed = 1`
  );

  // Calculate streak
  const workoutDates = await database.getAllAsync(
    `SELECT DISTINCT date(started_at) as d FROM workouts WHERE finished_at IS NOT NULL ORDER BY d DESC`
  );

  let streak = 0;
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  for (let i = 0; i < (workoutDates as any[]).length; i++) {
    const expectedDate = new Date(today);
    expectedDate.setDate(expectedDate.getDate() - i);
    const dateStr = expectedDate.toISOString().split('T')[0];
    if ((workoutDates as any[])[i]?.d === dateStr) {
      streak++;
    } else if (i === 0) {
      // Today might not have a workout yet, check yesterday
      continue;
    } else {
      break;
    }
  }

  return {
    totalWorkouts: countResult?.count || 0,
    totalVolume: volumeResult?.total || 0,
    currentStreak: streak,
  };
}

export async function getDailyStats(daysBack: number): Promise<{ date: string; volume: number; duration: number; count: number }[]> {
  const database = await getDatabase();
  const since = new Date();
  since.setDate(since.getDate() - daysBack);
  const sinceStr = since.toISOString().split('T')[0];

  const rows = await database.getAllAsync(
    `SELECT
       date(w.started_at) as date,
       COUNT(DISTINCT w.id) as count,
       COALESCE(SUM(w.duration_seconds), 0) as duration,
       COALESCE(SUM(ws.weight_kg * ws.reps), 0) as volume
     FROM workouts w
     LEFT JOIN workout_exercises we ON we.workout_id = w.id
     LEFT JOIN workout_sets ws ON ws.workout_exercise_id = we.id AND ws.completed = 1
     WHERE w.finished_at IS NOT NULL AND date(w.started_at) >= ?
     GROUP BY date(w.started_at)
     ORDER BY date(w.started_at) ASC`,
    [sinceStr]
  );
  return rows as any[];
}

// ============ CSV Export/Import ============

export async function exportWorkoutsCSV(): Promise<string> {
  const database = await getDatabase();
  const rows = await database.getAllAsync(
    `SELECT w.title, w.started_at, w.finished_at,
            e.name as exercise_name,
            ws.set_index, ws.set_type, ws.weight_kg, ws.reps,
            ws.distance_meters, ws.duration_seconds, ws.rpe
     FROM workouts w
     JOIN workout_exercises we ON we.workout_id = w.id
     JOIN exercises e ON e.id = we.exercise_id
     JOIN workout_sets ws ON ws.workout_exercise_id = we.id
     WHERE w.finished_at IS NOT NULL
     ORDER BY w.started_at, we.sort_order, ws.set_index`
  );

  let csv = 'title,start_time,end_time,exercise_name,set_index,set_type,weight_kg,reps,distance_meters,duration_seconds,rpe\n';
  for (const r of rows as any[]) {
    csv += `"${r.title}","${r.started_at}","${r.finished_at}","${r.exercise_name}",${r.set_index},"${r.set_type}",${r.weight_kg ?? ''},${r.reps ?? ''},${r.distance_meters ?? ''},${r.duration_seconds ?? ''},${r.rpe ?? ''}\n`;
  }
  return csv;
}
