import { useMemo } from 'react'
import { type SpaceId } from './SpaceRail'
import VaultRail from './VaultRail'

interface SidebarProps {
  vaultPath?: string | null;
  recentVaultPaths: string[];
  isSearchOpen?: boolean;
  activeSpace: SpaceId;
  entries: Entry[];
  noteTypes: NoteType[];
  currentEntry: Entry | null;
  onToggleCollapse: () => void;
  onSelectSpace: (spaceId: SpaceId) => void;
  onSelectVault: (vaultPath: string) => void;
  onOpenVaultPicker: () => void;
  onSearchToggle?: () => void;
  searchQuery: string;
  onCreateRootEntry: () => void;
  onOpenEntry: (entryId: string) => void;
  onOpenSettings: () => void;
}

const spaceTitles: Record<Exclude<SpaceId, 'my-space'>, string> = {
  'all-objects': 'Все объекты',
  'all-notes': 'Все заметки',
}

export default function Sidebar({
  vaultPath,
  recentVaultPaths,
  isSearchOpen,
  activeSpace,
  entries,
  noteTypes,
  currentEntry,
  onToggleCollapse,
  onSelectSpace,
  onSelectVault,
  onOpenVaultPicker,
  onSearchToggle,
  searchQuery,
  onCreateRootEntry,
  onOpenEntry,
  onOpenSettings,
}: SidebarProps) {
  const vaultName = useMemo(() => {
    if (!vaultPath) return 'Eden'
    const parts = vaultPath.split(/[/\\]/)
    return parts[parts.length - 1] || 'Eden'
  }, [vaultPath])

  const mySpaceEntry = useMemo(
    () => entries.find((entry) => entry.title.trim() === 'Мое пространство') ?? null,
    [entries],
  )

  const recentEntries = useMemo(
    () => [...entries].sort((a, b) => b.updated_at - a.updated_at).slice(0, 6),
    [entries],
  )

  const typedSections = useMemo(() => {
    return noteTypes
      .map((noteType) => ({
        noteType,
        entries: entries
          .filter((entry) => entry.type_id === noteType.id)
          .sort((a, b) => b.updated_at - a.updated_at)
          .slice(0, 4),
      }))
      .filter((section) => section.entries.length > 0)
      .slice(0, 4)
  }, [entries, noteTypes])

  const currentPageTitle = activeSpace === 'my-space' ? null : spaceTitles[activeSpace]

  return (
    <aside className="sidebar">
      <VaultRail
        vaultPath={vaultPath ?? null}
        recentVaultPaths={recentVaultPaths}
        onSelectVault={onSelectVault}
        onOpenVaultPicker={onOpenVaultPicker}
      />
      <button
        className="sidebar-top-toggle"
        data-testid="sidebar-toggle"
        onClick={onToggleCollapse}
        title="Скрыть сайдбар"
        type="button"
      >
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="4" width="18" height="16" rx="2"></rect>
          <path d="M9 4v16"></path>
        </svg>
      </button>
      <div className="sidebar-main">
        <div className="sidebar-scroll">
          <section className="sidebar-widget sidebar-widget-workspace">
            <div className="sidebar-widget-header">
              <div className="sidebar-workspace-meta">
                <span className="sidebar-workspace-avatar">{vaultName[0]?.toUpperCase() ?? 'E'}</span>
                <div>
                  <strong>{vaultName}</strong>
                  <p>Локальные виджеты</p>
                </div>
              </div>
              <div className="sidebar-header-actions">
                <button className="sidebar-action-btn" onClick={onCreateRootEntry} title="Новая заметка" type="button">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <line x1="12" y1="5" x2="12" y2="19"></line>
                    <line x1="5" y1="12" x2="19" y2="12"></line>
                  </svg>
                </button>
              </div>
            </div>
            <div className="sidebar-quick-links">
              <button className={`sidebar-quick-link ${isSearchOpen || searchQuery ? 'is-active' : ''}`} onClick={onSearchToggle} type="button">
                <span>⌕</span>
                <span>Поиск</span>
              </button>
              <button className={`sidebar-quick-link ${activeSpace === 'all-objects' ? 'is-active' : ''}`} onClick={() => onSelectSpace('all-objects')} type="button">
                <span>📚</span>
                <span>Виджет: Все объекты</span>
              </button>
            </div>
          </section>

          {currentPageTitle && (
            <button className="sidebar-widget sidebar-widget-page is-page" onClick={() => onSelectSpace(activeSpace)} type="button">
              <div className="sidebar-widget-title">{currentPageTitle}</div>
            </button>
          )}

          <button
            className={`sidebar-widget sidebar-widget-page ${currentEntry?.id === mySpaceEntry?.id ? 'is-current' : ''}`}
            onClick={() => onSelectSpace('my-space')}
            type="button"
          >
            <div className="sidebar-widget-title">🧑 Мое пространство</div>
          </button>

          <section className="sidebar-widget">
            <div className="sidebar-widget-header">
              <strong>Закрепленные виджеты</strong>
              <button className="sidebar-widget-more" type="button">+</button>
            </div>
            <p className="sidebar-widget-placeholder">
              Закрепленные виджеты появятся здесь. Пока это спокойная заглушка до следующего шага с pin-логикой.
            </p>
          </section>

          <section className="sidebar-widget">
            <div className="sidebar-widget-header">
              <strong>Недавние виджеты</strong>
            </div>
            <div className="sidebar-widget-list">
              {recentEntries.map((entry) => (
                <button
                  key={entry.id}
                  className={`sidebar-widget-list-item ${currentEntry?.id === entry.id ? 'is-current' : ''}`}
                  onClick={() => onOpenEntry(entry.id)}
                  type="button"
                >
                  <span className="sidebar-widget-item-icon">{entry.id === mySpaceEntry?.id ? '🧑' : '📄'}</span>
                  <span className="sidebar-widget-item-title">{entry.title || 'Без названия'}</span>
                </button>
              ))}
            </div>
          </section>

          {typedSections.map(({ noteType, entries: typedEntries }) => (
            <section className="sidebar-widget" key={noteType.id}>
              <div className="sidebar-widget-header">
                <strong>{noteType.name}</strong>
              </div>
              <div className="sidebar-widget-list">
                {typedEntries.map((entry) => (
                  <button
                    key={entry.id}
                    className={`sidebar-widget-list-item ${currentEntry?.id === entry.id ? 'is-current' : ''}`}
                    onClick={() => onOpenEntry(entry.id)}
                    type="button"
                  >
                    <span className="sidebar-widget-item-icon" style={{ color: noteType.color ?? undefined }}>{noteType.icon ?? '✦'}</span>
                    <span className="sidebar-widget-item-title">{entry.title || 'Без названия'}</span>
                  </button>
                ))}
              </div>
            </section>
          ))}
        </div>

        <div className="sidebar-footer-bar">
          <button className="sidebar-footer-btn" onClick={onOpenSettings} title="Открыть настройки" type="button">
            <span>⚙</span>
            <span>Settings</span>
          </button>
        </div>
      </div>
    </aside>
  )
}
