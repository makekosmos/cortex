import { useCallback, useEffect, useState } from 'react'

interface TrashSettingsProps {
  onRefreshData: () => Promise<void>;
}

function formatTimeAgo(deletedAt: number): string {
  const days = Math.floor((Date.now() - deletedAt) / (1000 * 60 * 60 * 24))
  if (days === 0) return 'сегодня'
  if (days === 1) return 'вчера'
  if (days < 7) return `${days} дн. назад`
  return `${Math.floor(days / 7)} нед. назад`
}

function daysRemaining(deletedAt: number): number {
  return Math.max(0, 30 - Math.floor((Date.now() - deletedAt) / (1000 * 60 * 60 * 24)))
}

export default function TrashSettings({ onRefreshData }: TrashSettingsProps) {
  const [trashEntries, setTrashEntries] = useState<Entry[]>([])
  const [loading, setLoading] = useState(true)

  const loadTrash = useCallback(async () => {
    if (!window.api?.listTrashEntries) return
    setLoading(true)
    const entries = await window.api.listTrashEntries()
    setTrashEntries(entries)
    setLoading(false)
  }, [])

  useEffect(() => { void loadTrash() }, [loadTrash])

  const handleRestore = async (entryId: string) => {
    await window.api.restoreEntry(entryId)
    await loadTrash()
    await onRefreshData()
  }

  const handlePermanentDelete = async (entryId: string) => {
    await window.api.permanentDeleteEntry(entryId)
    await loadTrash()
  }

  const handleEmptyTrash = async () => {
    for (const entry of trashEntries) {
      await window.api.permanentDeleteEntry(entry.id)
    }
    await loadTrash()
  }

  return (
    <div className="settings-tab">
      <div className="settings-tab-header-row">
        <div>
          <h1 className="settings-tab-title">Корзина</h1>
          <p className="settings-tab-subtitle">Удалённые заметки хранятся 30 дней, затем удаляются навсегда.</p>
        </div>
        {trashEntries.length > 0 && (
          <button className="settings-btn-danger" onClick={() => void handleEmptyTrash()} type="button">
            Очистить корзину
          </button>
        )}
      </div>

      <div className="settings-sections">
        {loading ? (
          <div className="settings-empty">Загрузка...</div>
        ) : trashEntries.length === 0 ? (
          <div className="settings-empty">Корзина пуста</div>
        ) : (
          <div className="trash-list">
            {trashEntries.map((entry) => (
              <div key={entry.id} className="trash-item">
                <div className="trash-item-info">
                  <img className="trash-item-icon" src="/anytype/icon/object/page.svg" alt="" width={18} height={18} draggable={false} />
                  <div className="trash-item-text">
                    <span className="trash-item-title">{entry.title || 'Без названия'}</span>
                    <span className="trash-item-meta">
                      Удалено {formatTimeAgo(entry.deleted_at!)} · осталось {daysRemaining(entry.deleted_at!)} дн.
                    </span>
                  </div>
                </div>
                <div className="trash-item-actions">
                  <button className="settings-btn-secondary" onClick={() => void handleRestore(entry.id)} type="button">
                    Восстановить
                  </button>
                  <button className="settings-btn-danger-sm" onClick={() => void handlePermanentDelete(entry.id)} type="button">
                    Удалить
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
