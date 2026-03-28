import { useMemo, useState } from 'react'

interface VaultRailProps {
  vaultPath: string | null;
  recentVaultPaths: string[];
  onSelectVault: (vaultPath: string) => void;
  onOpenVaultPicker: () => void;
}

function getVaultLabel(vaultPath: string) {
  const parts = vaultPath.split(/[/\\]/)
  return parts[parts.length - 1] || vaultPath
}

function getVaultInitial(vaultPath: string) {
  const label = getVaultLabel(vaultPath).trim()
  return label ? label[0]!.toUpperCase() : 'E'
}

export default function VaultRail({
  vaultPath,
  recentVaultPaths,
  onSelectVault,
  onOpenVaultPicker,
}: VaultRailProps) {
  const [isListOpen, setIsListOpen] = useState(true)
  const [filterQuery, setFilterQuery] = useState('')

  const visibleVaults = useMemo(() => {
    if (!vaultPath) {
      return []
    }

    const merged = [vaultPath, ...recentVaultPaths.filter((item) => item !== vaultPath)]
    return merged.slice(0, 12)
  }, [recentVaultPaths, vaultPath])

  const filteredVaults = useMemo(() => {
    const normalizedQuery = filterQuery.trim().toLowerCase()
    if (!normalizedQuery) {
      return visibleVaults
    }

    return visibleVaults.filter((item) => getVaultLabel(item).toLowerCase().includes(normalizedQuery))
  }, [filterQuery, visibleVaults])

  return (
    <div className="vault-rail-shell">
      <aside className="vault-rail" data-testid="vault-rail">
        <div className="vault-rail-list">
          {vaultPath && (
            <button
              className="vault-rail-item is-active"
              data-testid={`vault-item-${getVaultLabel(vaultPath)}`}
              onClick={() => onSelectVault(vaultPath)}
              title={getVaultLabel(vaultPath)}
              type="button"
            >
              <span>{getVaultInitial(vaultPath)}</span>
            </button>
          )}
        </div>
        <div className="vault-rail-actions">
          <button
            aria-label="Открыть список хранилищ"
            className="vault-rail-switcher"
            data-testid="vault-switcher-toggle"
            onClick={() => setIsListOpen((current) => !current)}
            title="Хранилища"
            type="button"
          >
            ≡
          </button>
        </div>
      </aside>

      {isListOpen && (
        <aside className="vault-browser" data-testid="vault-switcher-menu">
          <div className="vault-browser-header">
            <strong>Хранилища</strong>
            <div className="vault-browser-actions">
              <button
                className="vault-browser-action-btn"
                onClick={() => onOpenVaultPicker()}
                title="Добавить хранилище"
                type="button"
              >
                +
              </button>
              <button
                className="vault-browser-action-btn"
                onClick={() => setIsListOpen(false)}
                title="Скрыть список"
                type="button"
              >
                ×
              </button>
            </div>
          </div>

          <input
            className="vault-browser-search"
            onChange={(event) => setFilterQuery(event.target.value)}
            placeholder="Поиск хранилищ"
            type="text"
            value={filterQuery}
          />

          <div className="vault-browser-list">
            {filteredVaults.map((item) => (
              <button
                key={item}
                className={`vault-browser-item ${item === vaultPath ? 'is-active' : ''}`}
                onClick={() => onSelectVault(item)}
                type="button"
              >
                <span className="vault-browser-item-avatar">{getVaultInitial(item)}</span>
                <span className="vault-browser-item-name">{getVaultLabel(item)}</span>
              </button>
            ))}

            {filteredVaults.length === 0 && (
              <div className="vault-browser-empty">Ничего не найдено</div>
            )}
          </div>
        </aside>
      )}
    </div>
  )
}
