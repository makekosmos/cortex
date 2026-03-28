import './SpacesView.css'
import { useMemo } from 'react'
import type { SpaceId } from '@/components/sidebar/SpaceRail'
import { sortEntries, type SortMode } from '@/components/sidebar/types'
import { SYSTEM_TYPE_WORKOUT_ID, SYSTEM_TYPE_EXERCISE_ID } from '@/lib/systemTypes'

interface SpacesViewProps {
  activeSpace: SpaceId;
  entries: Entry[];
  noteTypes: NoteType[];
  sortMode: SortMode;
  onSortModeChange: (sortMode: SortMode) => void;
  onCreateEntry: () => void;
  onOpenEntry: (entryId: string) => void;
}

interface ObjectRow {
  id: string;
  title: string;
  subtitle: string;
  updatedAt: number;
}

function formatDate(timestamp: number) {
  return new Date(timestamp).toLocaleDateString('ru-RU', {
    day: 'numeric',
    month: 'short',
  })
}

function formatFullDate(timestamp: number) {
  return new Date(timestamp).toLocaleDateString('ru-RU', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  })
}

function getHeaderProp(entry: Entry, key: string): string {
  try {
    const props = JSON.parse(entry.header_props_json || '{}')
    return String(props[key] ?? '')
  } catch {
    return ''
  }
}

function todayDateString(): string {
  const d = new Date()
  return d.toISOString().split('T')[0]
}

export default function SpacesView({
  activeSpace,
  entries,
  noteTypes,
  sortMode,
  onSortModeChange,
  onCreateEntry,
  onOpenEntry,
}: SpacesViewProps) {
  const sortedEntries = useMemo(
    () => sortEntries(entries, sortMode),
    [entries, sortMode],
  )

  const allObjects = useMemo<ObjectRow[]>(
    () => entries.map((entry) => {
      const noteType = entry.type_id ? noteTypes.find((nt) => nt.id === entry.type_id) : null
      return {
        id: entry.id,
        title: entry.title || 'Без названия',
        subtitle: noteType?.name ?? 'Страница',
        updatedAt: entry.updated_at,
      }
    }).sort((a, b) => b.updatedAt - a.updatedAt),
    [entries, noteTypes],
  )

  const renderEmptyBlock = (title: string, description: string, actionLabel?: string, onAction?: () => void) => (
    <div className="space-empty-block">
      <h3>{title}</h3>
      <p>{description}</p>
      {actionLabel && onAction && (
        <button className="space-primary-btn" onClick={onAction} type="button">
          {actionLabel}
        </button>
      )}
    </div>
  )

  if (activeSpace === 'my-space') {
    return (
      <section className="spaces-view" data-testid="space-view-my-space">
        <div className="space-empty-block">
          <h3>Открываю заметку пространства</h3>
          <p>Это пространство является обычной заметкой, а не отдельным обзорным экраном.</p>
        </div>
      </section>
    )
  }

  if (activeSpace === 'all-objects') {
    return (
      <section className="spaces-view" data-testid="space-view-all-objects">
        <div className="space-page-header">
          <div>
            <p className="space-eyebrow">Обзор</p>
            <h1>Все объекты</h1>
            <p className="space-page-text">Единая таблица заметок и объектных свойств, отсортированная по последним изменениям.</p>
          </div>
        </div>
        <section className="space-panel">
          {allObjects.length > 0 ? (
            <div className="space-table">
              <div className="space-table-head">
                <span>Название</span>
                <span>Тип</span>
                <span>Обновлено</span>
              </div>
              {allObjects.map((row) => (
                <button
                  key={row.id}
                  className="space-table-row"
                  onClick={() => onOpenEntry(row.id)}
                  type="button"
                >
                  <span>{row.title}</span>
                  <span>{row.subtitle}</span>
                  <time>{formatDate(row.updatedAt)}</time>
                </button>
              ))}
            </div>
          ) : renderEmptyBlock('Нет объектов', 'Добавь заметки или объектные свойства, чтобы здесь появилась таблица.', 'Создать заметку', onCreateEntry)}
        </section>
      </section>
    )
  }

  if (activeSpace === 'all-properties') {
    const propertyTypes = noteTypes.filter(nt => nt.id !== 'system-type-page')
    return (
      <section className="spaces-view" data-testid="space-view-all-properties">
        <div className="space-page-header">
          <div>
            <p className="space-eyebrow">Модель данных</p>
            <h1>Все свойства</h1>
            <p className="space-page-text">Типы объектов и их свойства.</p>
          </div>
        </div>
        <section className="space-panel">
          {propertyTypes.length > 0 ? (
            <div className="space-table">
              <div className="space-table-head">
                <span>Тип</span>
                <span>Slug</span>
                <span>Объектов</span>
              </div>
              {propertyTypes.map((nt) => {
                const count = entries.filter(e => e.type_id === nt.id).length
                return (
                  <div key={nt.id} className="space-table-row">
                    <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <img
                        src={`/anytype/icon/type/default/${nt.icon || 'document'}.svg`}
                        alt=""
                        width={16}
                        height={16}
                        draggable={false}
                      />
                      {nt.name}
                    </span>
                    <span>{nt.slug}</span>
                    <span>{count}</span>
                  </div>
                )
              })}
            </div>
          ) : renderEmptyBlock('Нет типов', 'Создайте типы объектов в настройках.')}
        </section>
      </section>
    )
  }

  if (activeSpace === 'all-notes') {
    return (
      <section className="spaces-view" data-testid="space-view-all-notes">
        <div className="space-page-header">
          <div>
            <p className="space-eyebrow">Коллекция</p>
            <h1>Все заметки</h1>
            <p className="space-page-text">Список заметок теперь открывается как отдельная страница, без внутреннего уровня пространства в сайдбаре.</p>
          </div>
          <div className="space-page-actions">
            <label className="space-sort-control">
              <span>Сортировка</span>
              <select value={sortMode} onChange={(event) => onSortModeChange(event.target.value as SortMode)}>
                <option value="updated_at">По изменению</option>
                <option value="created_at">По созданию</option>
                <option value="title">По названию</option>
              </select>
            </label>
            <button className="space-primary-btn" onClick={onCreateEntry} type="button">
              Новая заметка
            </button>
          </div>
        </div>
        {sortedEntries.length > 0 ? (
          <div className="space-table">
            <div className="space-table-head">
              <span>Название</span>
              <span>Тип</span>
              <span>Обновлено</span>
            </div>
            {sortedEntries.map((entry) => (
              <button
                key={entry.id}
                className="space-table-row"
                onClick={() => onOpenEntry(entry.id)}
                type="button"
              >
                <span>{entry.title || 'Без названия'}</span>
                <span>{entry.type_id ? 'С объектным свойством' : 'Обычная заметка'}</span>
                <time>{formatDate(entry.updated_at)}</time>
              </button>
            ))}
          </div>
        ) : renderEmptyBlock('Нет заметок', 'Создай первую заметку, и здесь появится полноценная таблица.', 'Создать заметку', onCreateEntry)}
      </section>
    )
  }

  if (activeSpace === 'diary') {
    const today = todayDateString()
    const todayFormatted = formatFullDate(Date.now())

    const todayWorkouts = entries.filter(e =>
      e.type_id === SYSTEM_TYPE_WORKOUT_ID && getHeaderProp(e, 'date') === today
    ).sort((a, b) => b.created_at - a.created_at)

    const todayExercises = entries.filter(e =>
      e.type_id === SYSTEM_TYPE_EXERCISE_ID
    ).filter(e => {
      const workoutEntry = todayWorkouts.find(w => e.id.startsWith(`hevy-exercise-${getHeaderProp(w, 'hevy_id')}`))
      return workoutEntry !== undefined
    })

    const allWorkouts = entries.filter(e =>
      e.type_id === SYSTEM_TYPE_WORKOUT_ID
    ).sort((a, b) => b.created_at - a.created_at)

    return (
      <section className="spaces-view" data-testid="space-view-diary">
        <div className="space-page-header">
          <div>
            <p className="space-eyebrow">Дневник</p>
            <h1>{todayFormatted}</h1>
            <p className="space-page-text">Записи за сегодня и история тренировок.</p>
          </div>
        </div>

        <section className="space-panel">
          <h2 className="diary-section-title" data-testid="diary-today-section">Сегодня</h2>
          {todayWorkouts.length > 0 ? (
            <div className="space-table">
              <div className="space-table-head">
                <span>Тренировка</span>
                <span>Длительность</span>
                <span>Объём</span>
                <span>Упражнений</span>
              </div>
              {todayWorkouts.map((entry) => (
                <button
                  key={entry.id}
                  className="space-table-row"
                  onClick={() => onOpenEntry(entry.id)}
                  type="button"
                  data-testid="diary-workout-row"
                >
                  <span>{entry.title}</span>
                  <span>{getHeaderProp(entry, 'duration_min')} мин</span>
                  <span>{getHeaderProp(entry, 'volume_kg')} кг</span>
                  <span>{getHeaderProp(entry, 'exercise_count')}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className="space-empty-block">
              <p>Сегодня тренировок пока нет. Синхронизируйте данные из связанных программ.</p>
            </div>
          )}

          {todayExercises.length > 0 && (
            <>
              <h3 className="diary-section-subtitle">Упражнения за сегодня</h3>
              <div className="space-table">
                <div className="space-table-head">
                  <span>Упражнение</span>
                  <span>Мышцы</span>
                  <span>Лучший подход</span>
                  <span>Объём</span>
                </div>
                {todayExercises.map((entry) => (
                  <button
                    key={entry.id}
                    className="space-table-row"
                    onClick={() => onOpenEntry(entry.id)}
                    type="button"
                    data-testid="diary-exercise-row"
                  >
                    <span>{getHeaderProp(entry, 'exercise_name')}</span>
                    <span>{getHeaderProp(entry, 'muscle_group')}</span>
                    <span>{getHeaderProp(entry, 'best_set')}</span>
                    <span>{getHeaderProp(entry, 'total_volume_kg')} кг</span>
                  </button>
                ))}
              </div>
            </>
          )}
        </section>

        <section className="space-panel">
          <h2 className="diary-section-title" data-testid="diary-history-section">История тренировок</h2>
          {allWorkouts.length > 0 ? (
            <div className="space-table">
              <div className="space-table-head">
                <span>Тренировка</span>
                <span>Дата</span>
                <span>Длительность</span>
                <span>Объём</span>
              </div>
              {allWorkouts.map((entry) => (
                <button
                  key={entry.id}
                  className="space-table-row"
                  onClick={() => onOpenEntry(entry.id)}
                  type="button"
                  data-testid="diary-history-row"
                >
                  <span>{entry.title}</span>
                  <time>{getHeaderProp(entry, 'date')}</time>
                  <span>{getHeaderProp(entry, 'duration_min')} мин</span>
                  <span>{getHeaderProp(entry, 'volume_kg')} кг</span>
                </button>
              ))}
            </div>
          ) : renderEmptyBlock(
            'Нет тренировок',
            'Подключите Hevy в настройках (Связанные программы) и синхронизируйте тренировки.'
          )}
        </section>
      </section>
    )
  }

  return null
}
