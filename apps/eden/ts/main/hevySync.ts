import type { Entry } from './store'
import { parseHevyWorkout, type HevyWorkout, type HevyExercise } from '@/lib/hevy'

const SYSTEM_TYPE_WORKOUT_ID = 'system-type-workout'
const SYSTEM_TYPE_EXERCISE_ID = 'system-type-exercise'

function formatDuration(startTime: number, endTime: number): number {
  return Math.round((endTime - startTime) / 60)
}

function formatDate(timestamp: number): string {
  const d = new Date(timestamp * 1000)
  return d.toISOString().split('T')[0]
}

function formatSetsSummary(exercise: HevyExercise): string {
  return exercise.sets.map((set, i) => {
    const parts: string[] = []
    if (set.indicator !== 'normal') {
      parts.push(`[${set.indicator}]`)
    }
    parts.push(`${i + 1}.`)
    if (set.weightKg !== null) {
      parts.push(`${set.weightKg} кг`)
    }
    if (set.reps !== null) {
      parts.push(`× ${set.reps}`)
    }
    if (set.distanceMeters !== null) {
      parts.push(`${set.distanceMeters} м`)
    }
    if (set.durationSeconds !== null) {
      const min = Math.floor(set.durationSeconds / 60)
      const sec = set.durationSeconds % 60
      parts.push(`${min}:${String(sec).padStart(2, '0')}`)
    }
    if (set.rpe !== null) {
      parts.push(`RPE ${set.rpe}`)
    }
    return parts.join(' ')
  }).join('\n')
}

function findBestSet(exercise: HevyExercise): string {
  let best = ''
  let bestWeight = 0
  for (const set of exercise.sets) {
    if (set.indicator === 'warmup') continue
    const w = set.weightKg ?? 0
    if (w > bestWeight) {
      bestWeight = w
      const parts: string[] = []
      if (set.weightKg !== null) parts.push(`${set.weightKg} кг`)
      if (set.reps !== null) parts.push(`× ${set.reps}`)
      best = parts.join(' ')
    }
  }
  if (!best && exercise.sets.length > 0) {
    const s = exercise.sets[0]
    const parts: string[] = []
    if (s.reps !== null) parts.push(`${s.reps} повт.`)
    if (s.durationSeconds !== null) {
      const min = Math.floor(s.durationSeconds / 60)
      parts.push(`${min} мин`)
    }
    best = parts.join(' ')
  }
  return best
}

function calcExerciseVolume(exercise: HevyExercise): number {
  let vol = 0
  for (const set of exercise.sets) {
    if (set.weightKg && set.reps) {
      vol += set.weightKg * set.reps
    }
  }
  return Math.round(vol * 10) / 10
}

function workoutContentJson(workout: HevyWorkout): string {
  const blocks: Array<Record<string, unknown>> = []

  if (workout.description) {
    blocks.push({
      type: 'paragraph',
      content: [{ type: 'text', text: workout.description }],
    })
  }

  for (const exercise of workout.exercises) {
    blocks.push({
      type: 'heading',
      attrs: { level: 3 },
      content: [{ type: 'text', text: exercise.title }],
    })

    const info: string[] = []
    if (exercise.muscleGroup && exercise.muscleGroup !== 'other') {
      info.push(`Группа мышц: ${exercise.muscleGroup}`)
    }
    if (exercise.equipmentCategory && exercise.equipmentCategory !== 'other' && exercise.equipmentCategory !== 'none') {
      info.push(`Оборудование: ${exercise.equipmentCategory}`)
    }
    if (info.length > 0) {
      blocks.push({
        type: 'paragraph',
        content: [{ type: 'text', marks: [{ type: 'italic' }], text: info.join(' · ') }],
      })
    }

    for (const set of exercise.sets) {
      const parts: string[] = []
      if (set.indicator !== 'normal') parts.push(`[${set.indicator}]`)
      if (set.weightKg !== null) parts.push(`${set.weightKg} кг`)
      if (set.reps !== null) parts.push(`× ${set.reps} повт.`)
      if (set.distanceMeters !== null) parts.push(`${set.distanceMeters} м`)
      if (set.durationSeconds !== null) {
        const min = Math.floor(set.durationSeconds / 60)
        const sec = set.durationSeconds % 60
        parts.push(`${min}:${String(sec).padStart(2, '0')}`)
      }
      if (set.rpe !== null) parts.push(`RPE ${set.rpe}`)

      blocks.push({
        type: 'paragraph',
        content: [{ type: 'text', text: parts.join(' ') }],
      })
    }

    if (exercise.notes) {
      blocks.push({
        type: 'paragraph',
        content: [{ type: 'text', marks: [{ type: 'italic' }], text: `Заметка: ${exercise.notes}` }],
      })
    }
  }

  if (blocks.length === 0) {
    blocks.push({ type: 'paragraph' })
  }

  return JSON.stringify({ type: 'doc', content: blocks })
}

export interface SyncResult {
  workoutsCreated: number
  workoutsSkipped: number
  exerciseEntriesCreated: number
}

export function convertHevyWorkoutsToEntries(
  rawWorkouts: unknown[],
  existingEntries: Entry[],
): { workoutEntries: Entry[]; exerciseEntries: Entry[]; stats: SyncResult } {
  const existingHevyIds = new Set<string>()
  for (const entry of existingEntries) {
    try {
      const props = JSON.parse(entry.header_props_json || '{}')
      if (props.hevy_id) {
        existingHevyIds.add(props.hevy_id)
      }
    } catch {
      // skip
    }
  }

  const workoutEntries: Entry[] = []
  const exerciseEntries: Entry[] = []
  let workoutsSkipped = 0

  for (const raw of rawWorkouts) {
    const workout = parseHevyWorkout(raw as Record<string, unknown>)

    if (existingHevyIds.has(workout.id)) {
      workoutsSkipped++
      continue
    }

    const dateStr = formatDate(workout.startTime)
    const durationMin = formatDuration(workout.startTime, workout.endTime)
    const createdMs = workout.startTime * 1000

    const workoutEntry: Entry = {
      id: `hevy-workout-${workout.id}`,
      title: `${workout.name || 'Тренировка'} — ${dateStr}`,
      content_json: workoutContentJson(workout),
      created_at: createdMs,
      updated_at: createdMs,
      folder_id: null,
      type_id: SYSTEM_TYPE_WORKOUT_ID,
      header_layout: 'default',
      header_props_json: JSON.stringify({
        date: dateStr,
        duration_min: String(durationMin),
        volume_kg: String(Math.round(workout.estimatedVolumeKg)),
        exercise_count: String(workout.exercises.length),
        source: 'Hevy',
        hevy_id: workout.id,
      }),
      schema_version: 1,
      deleted_at: null,
    }
    workoutEntries.push(workoutEntry)

    for (const exercise of workout.exercises) {
      const exerciseEntry: Entry = {
        id: `hevy-exercise-${workout.id}-${exercise.id}`,
        title: `${exercise.title} — ${dateStr}`,
        content_json: JSON.stringify({
          type: 'doc',
          content: [{
            type: 'paragraph',
            content: [{ type: 'text', text: formatSetsSummary(exercise) || 'Нет данных по подходам' }],
          }],
        }),
        created_at: createdMs,
        updated_at: createdMs,
        folder_id: null,
        type_id: SYSTEM_TYPE_EXERCISE_ID,
        header_layout: 'default',
        header_props_json: JSON.stringify({
          exercise_name: exercise.title,
          exercise_type: exercise.exerciseType,
          equipment: exercise.equipmentCategory,
          muscle_group: exercise.muscleGroup,
          sets_summary: formatSetsSummary(exercise),
          best_set: findBestSet(exercise),
          total_volume_kg: String(calcExerciseVolume(exercise)),
          notes: exercise.notes || '',
        }),
        schema_version: 1,
        deleted_at: null,
      }
      exerciseEntries.push(exerciseEntry)
    }
  }

  return {
    workoutEntries,
    exerciseEntries,
    stats: {
      workoutsCreated: workoutEntries.length,
      workoutsSkipped,
      exerciseEntriesCreated: exerciseEntries.length,
    },
  }
}
