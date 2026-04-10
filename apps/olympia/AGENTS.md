# Olympia

Workout tracker app — part of the Kosmos ecosystem. Built with Expo (SDK 54), React Native, TypeScript.

## Architecture

### Stack
- **Framework**: Expo Router (file-based routing)
- **State**: Zustand stores with SQLite persistence (`expo-sqlite`)
- **Styling**: StyleSheet (dark/light theme via `constants/Colors.ts`)
- **IDs**: `uuid` for entity generation

### Key Directories
```
app/                       # Expo Router pages
  (tabs)/                  # Tab screens: workouts (index), profile
    index.tsx              # Workout history list + start workout
    profile.tsx            # Body weight log, settings, data export
  workout/
    live.tsx               # Active workout screen (full-screen modal)
    add-exercise.tsx       # Exercise picker (modal)
    [id].tsx               # Workout details
  exercise/[id].tsx        # Exercise info + history
  routine/
    create.tsx             # Create/edit routine
    [id].tsx               # Routine details
  stats/strength.tsx       # Strength progression charts
lib/
  database.ts              # SQLite schema, CRUD helpers, seed data
  types.ts                 # TypeScript interfaces + muscle/equipment constants
  id.ts                    # UUID generator
  useThemeColor.ts         # Theme-aware color hook
  stores/
    workout-store.ts       # Active workout state (exercises, sets, superset, rest timer)
    exercises-store.ts     # Exercise catalog (seeded + custom)
    settings-store.ts      # Theme, units, preferences
components/                # Hooks (useColorScheme, useClientOnlyValue)
constants/Colors.ts        # Dark + light color palettes
```

### Data Flow
1. `_layout.tsx` initializes DB, seeds exercises, loads settings and exercise catalog
2. Stores hydrate from SQLite on app start
3. Mutations write to SQLite first, then update Zustand state
4. Components subscribe to Zustand state reactively

### Database Schema (SQLite)
- `exercises` — id, name, force, level, mechanic, equipment, primary_muscles, secondary_muscles, instructions, images, category, is_custom
- `workouts` — id, title, started_at, finished_at, notes, duration_seconds
- `workout_exercises` — id, workout_id, exercise_id, sort_order, notes
- `workout_sets` — id, workout_exercise_id, set_index, set_type, weight_kg, reps, duration_seconds, distance_meters, completed, rpe
- `routines` — id, title, created_at, updated_at
- `routine_exercises` — id, routine_id, exercise_id, sort_order, target_sets, target_reps, target_weight_kg, notes
- `body_weight_log` — id, date, weight_kg, notes

## Conventions
- Language in UI: Russian
- Package manager: bun (bun.lock)
- Path aliases: `@/` maps to project root
- Set types: normal, warmup, dropset, failure
- Weight in kg, RPE scale for intensity
- Muscle/equipment names stored in English, displayed in Russian via `muscleRu()`
- Dark theme is default; light theme supported
- All colors defined in `constants/Colors.ts`
