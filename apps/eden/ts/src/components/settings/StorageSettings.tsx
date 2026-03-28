import { useState } from 'react'

interface StorageSettingsProps {
  vaultPath: string;
}

interface StorageFile {
  id: string;
  name: string;
  size: number;
  type: 'image' | 'file';
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(2)} ${units[i]}`
}

const TOTAL_CAPACITY = 20 * 1024 * 1024 * 1024 // 20 GB

const MOCK_FILES: StorageFile[] = [
  { id: '1', name: '1000044850.png', size: 2.09 * 1024 * 1024, type: 'image' },
  { id: '2', name: 'image_1773323557702_0.png', size: 802.23 * 1024, type: 'image' },
  { id: '3', name: '1000045388.webp', size: 554.3 * 1024, type: 'image' },
  { id: '4', name: 'yd5mi4z4e20hytl9tmcporvs2zugx9y7.jpeg', size: 402.35 * 1024, type: 'image' },
  { id: '5', name: '1000045401.jpg', size: 362.43 * 1024, type: 'image' },
  { id: '6', name: 'deniel-kiz.jpg', size: 90.19 * 1024, type: 'image' },
  { id: '7', name: '00044830.jpg', size: 31.46 * 1024, type: 'image' },
  { id: '8', name: '00044830.jpg', size: 19.54 * 1024, type: 'image' },
  { id: '9', name: 'lab721_com_br_icon.ico', size: 15.04 * 1024, type: 'file' },
]

export default function StorageSettings({ vaultPath }: StorageSettingsProps) {
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [filterOpen, setFilterOpen] = useState(false)

  const currentSpaceBytes = 60.03 * 1024 * 1024
  const otherSpacesBytes = 5.1 * 1024 * 1024
  const totalUsed = currentSpaceBytes + otherSpacesBytes

  const currentPercent = (currentSpaceBytes / TOTAL_CAPACITY) * 100
  const otherPercent = (otherSpacesBytes / TOTAL_CAPACITY) * 100
  const freePercent = 100 - currentPercent - otherPercent

  const spaceName = vaultPath.split(/[/\\]/).pop() || 'Eden'

  const filteredFiles = MOCK_FILES.filter((f) =>
    f.name.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const allSelected = filteredFiles.length > 0 && filteredFiles.every((f) => selectedIds.has(f.id))

  const toggleSelectAll = () => {
    if (allSelected) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(filteredFiles.map((f) => f.id)))
    }
  }

  const toggleFile = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <div className="settings-tab">
      <h1 className="settings-tab-title">Облачное хранилище</h1>
      <p className="settings-tab-subtitle">
        Вы можете хранить ваши файлы на нашем зашифрованном узле резервного копирования. Как только вы достигнете лимита, файлы перестанут синхронизироваться и будут храниться только локально.
      </p>

      <div className="settings-sections">
        <div className="storage-usage-wrapper">
          <div className="storage-progress-bar">
            <div className="storage-bar-track">
              <div className="storage-bar-segment current" style={{ width: `${Math.max(currentPercent, 0.3)}%` }} />
              <div className="storage-bar-segment other" style={{ width: `${Math.max(otherPercent, 0.2)}%` }} />
              <div className="storage-bar-segment empty" style={{ width: `${freePercent}%` }} />
            </div>
          </div>

          <div className="storage-usage-info">
            <div className="storage-usage-total">
              <span className="storage-usage-used">{formatBytes(totalUsed)} </span>
              {formatBytes(TOTAL_CAPACITY)} использовано
            </div>
            <div className="storage-usage-legend">
              <div className="storage-legend-entry">
                <span className="storage-legend-marker current" />
                {spaceName}
              </div>
              <div className="storage-legend-entry">
                <span className="storage-legend-marker other" />
                Другие пространства
              </div>
              <div className="storage-legend-entry">
                <span className="storage-legend-marker free" />
                Свободное место
              </div>
            </div>
          </div>
        </div>

        <div className="storage-file-manager">
          <div className="storage-tabs">
            <div className="storage-tab-item active">Синхронизировано</div>
          </div>

          <div className="storage-controls">
            <div className="storage-controls-left">
              <button className="storage-checkbox-btn" onClick={toggleSelectAll} type="button">
                <span className={`storage-checkbox-icon ${allSelected ? 'checked' : ''}`} />
                <span>Выбрать всё</span>
              </button>
            </div>
            <div className="storage-controls-right">
              <button className="storage-icon-btn" onClick={() => setFilterOpen(!filterOpen)} type="button">
                <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                  <circle cx="8.5" cy="8.5" r="5.5" />
                  <path d="M13 13L17 17" />
                </svg>
              </button>
              <div className={`storage-filter-wrap ${filterOpen ? 'active' : ''}`}>
                <input
                  className="storage-filter-input"
                  type="text"
                  placeholder="Поиск..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                />
              </div>
            </div>
          </div>

          <div className="storage-file-list">
            {filteredFiles.map((file) => (
              <div className="storage-file-row" key={file.id}>
                <button
                  className={`storage-checkbox-icon ${selectedIds.has(file.id) ? 'checked' : ''}`}
                  onClick={() => toggleFile(file.id)}
                  type="button"
                />
                <div className="storage-file-click-area">
                  <div className={`storage-file-icon ${file.type === 'image' ? 'is-image' : 'is-file'}`}>
                    {file.type === 'image' ? (
                      <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.2">
                        <rect x="2" y="2" width="14" height="14" rx="2" />
                        <circle cx="6.5" cy="6.5" r="1.5" />
                        <path d="M2 13L6 9L10 13" />
                        <path d="M9 11L12 8L16 12" />
                      </svg>
                    ) : (
                      <svg width="18" height="18" viewBox="0 0 18 18" fill="none" stroke="currentColor" strokeWidth="1.2">
                        <path d="M4 2H11L14 5V16H4V2Z" />
                        <path d="M11 2V5H14" />
                      </svg>
                    )}
                  </div>
                  <div className="storage-file-info">
                    <div className="storage-file-name"><span>{file.name}</span></div>
                    <div className="storage-file-size">{formatBytes(file.size)}</div>
                  </div>
                </div>
              </div>
            ))}
            {filteredFiles.length === 0 && (
              <div className="storage-file-empty">Ничего не найдено</div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
