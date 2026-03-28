import { type MouseEvent as ReactMouseEvent, useCallback, useEffect, useRef, useState } from 'react'
import { v4 as uuidv4 } from 'uuid'
import SearchOverlay from '@/components/SearchOverlay'
import { type SpaceId } from '@/components/sidebar/SpaceRail'
import { type SortMode } from '@/components/sidebar/types'
import VaultSidebar from '@/components/sidebar/VaultSidebar'
import WidgetSidebar from '@/components/sidebar/WidgetSidebar'
import SpacesView from '@/components/spaces/SpacesView'
import Editor from '@/Editor'
import SettingsPage from '@/components/settings/SettingsPage'
import Titlebar from '@/Titlebar'
import { normalizeSlug } from '@/lib/typedNotes'
import { SYSTEM_TYPES, isSystemType } from '@/lib/systemTypes'
import '@/App.css'

type ActiveScreen = 'notes' | 'settings'

interface QueuedSaveRequest {
  entry: Entry;
  waiters: Array<{
    resolve: (result: SaveEntryResult | null) => void;
    reject: (error: unknown) => void;
  }>;
}

interface EntrySaveCoordinator {
  inFlight: boolean;
  queued: QueuedSaveRequest | null;
}

const MIN_WIDGET_SIDEBAR_WIDTH = 220
const MAX_WIDGET_SIDEBAR_WIDTH = 520
const MIN_VAULT_SIDEBAR_WIDTH = 180
const MAX_VAULT_SIDEBAR_WIDTH = 360
const COLLAPSE_THRESHOLD = 60
const MY_SPACE_TITLE = 'Мое пространство'

export default function App() {
  const [pendingSearchQuery, setPendingSearchQuery] = useState('')
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [activeScreen, setActiveScreen] = useState<ActiveScreen>('notes')
  const [activeSpace, setActiveSpace] = useState<SpaceId>('my-space')
  const [vaultPath, setVaultPath] = useState<string | null>(null)
  const [recentVaultPaths, setRecentVaultPaths] = useState<string[]>([])
  const [isInitializing, setIsInitializing] = useState(true)
  const [codeToolsSettings, setCodeToolsSettings] = useState<CodeToolsSettings | null>(null)
  const [entries, setEntries] = useState<Entry[]>([])
  const [noteTypes, setNoteTypes] = useState<NoteType[]>([])
  const [currentEntry, setCurrentEntry] = useState<Entry | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<SearchResult[]>([])
  const [sortMode, setSortMode] = useState<SortMode>('updated_at')
  const [widgetSidebarWidth, setWidgetSidebarWidth] = useState(320)
  const [widgetSidebarCollapsed, setWidgetSidebarCollapsed] = useState(false)
  const [vaultSidebarWidth, setVaultSidebarWidth] = useState(232)
  const [vaultSidebarCollapsed, setVaultSidebarCollapsed] = useState(false)
  const [activeResizePanel, setActiveResizePanel] = useState<'vault' | 'widget' | null>(null)
  const [sidebarAnimating, setSidebarAnimating] = useState(false)

  const resizeRef = useRef<number | null>(null)
  const animTimerRef = useRef<number | null>(null)
  const isAnimatingRef = useRef(false)
  const latestSaveTimestampRef = useRef<Record<string, number>>({})
  const saveCoordinatorsRef = useRef<Record<string, EntrySaveCoordinator>>({})

  // --- Detect platform and add class to <html> ---
  useEffect(() => {
    if (!window.api?.getPlatform) return
    void window.api.getPlatform().then((platform: string) => {
      if (platform === 'darwin') {
        document.documentElement.classList.add('platform-mac')
      } else if (platform === 'win32') {
        document.documentElement.classList.add('platform-windows')
      }
    })
  }, [])

  // --- Native Electron Zoom (Ctrl+=/Ctrl+-/Ctrl+0, persisted in localStorage) ---
  useEffect(() => {
    const STEP = 0.1
    const MIN = 0.5
    const MAX = 2.0

    const restore = async () => {
      const saved = localStorage.getItem('eden-zoom')
      if (saved && window.api?.zoomSet) {
        await window.api.zoomSet(parseFloat(saved))
      }
    }
    void restore()

    const handleKey = async (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || !window.api?.zoomGet) return

      let next: number | null = null

      if (e.key === '=' || e.key === '+') {
        const cur = await window.api.zoomGet()
        next = Math.min(MAX, Math.round((cur + STEP) * 100) / 100)
      } else if (e.key === '-') {
        const cur = await window.api.zoomGet()
        next = Math.max(MIN, Math.round((cur - STEP) * 100) / 100)
      } else if (e.key === '0') {
        next = 1
      }

      if (next !== null) {
        e.preventDefault()
        const applied = await window.api.zoomSet(next)
        localStorage.setItem('eden-zoom', String(applied))
      }
    }

    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [])

  const refreshData = useCallback(async () => {
    if (!window.api) return

    const [entriesData, noteTypesData] = await Promise.all([
      window.api.listEntries(),
      window.api.listNoteTypes(),
    ])

    setEntries(entriesData)
    setNoteTypes([...SYSTEM_TYPES, ...noteTypesData])
    setCurrentEntry((prevEntry) => prevEntry ? (entriesData.find((entry) => entry.id === prevEntry.id) ?? prevEntry) : null)
  }, [])

  useEffect(() => {
    const initApp = async () => {
      if (!window.api) return

      const [path, recentPaths, settings, sidebarConfig] = await Promise.all([
        window.api.getVaultPath(),
        window.api.getRecentVaultPaths(),
        window.api.getCodeToolsSettings(),
        window.api.getSidebarConfig(),
      ])

      setVaultPath(path)
      setRecentVaultPaths(recentPaths)
      setCodeToolsSettings(settings)
      setWidgetSidebarWidth(sidebarConfig.widget.width)
      setWidgetSidebarCollapsed(sidebarConfig.widget.collapsed)
      setVaultSidebarWidth(sidebarConfig.vault.width)
      setVaultSidebarCollapsed(sidebarConfig.vault.collapsed)

      if (path) {
        const [entriesData, noteTypesData] = await Promise.all([
          window.api.listEntries(),
          window.api.listNoteTypes(),
        ])

        setEntries(entriesData)
        setNoteTypes([...SYSTEM_TYPES, ...noteTypesData])

        const existingMySpace = entriesData.find((entry) => entry.title.trim() === MY_SPACE_TITLE) ?? null
        if (existingMySpace) {
          setCurrentEntry(existingMySpace)
        } else {
          const mySpaceEntry: Entry = {
            id: uuidv4(),
            title: MY_SPACE_TITLE,
            content_json: JSON.stringify({ type: 'doc', content: [{ type: 'paragraph' }] }),
            created_at: Date.now(),
            updated_at: Date.now(),
            folder_id: null,
            type_id: null,
            header_layout: null,
            header_props_json: '{}',
            schema_version: 1,
            deleted_at: null,
          }
          setEntries((prevEntries) => [mySpaceEntry, ...prevEntries])
          void window.api.saveEntry(mySpaceEntry)
          setCurrentEntry(mySpaceEntry)
        }
      }

      setIsInitializing(false)
    }

    void initApp()
  }, [])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSearchQuery(pendingSearchQuery)
    }, 300)

    return () => window.clearTimeout(timer)
  }, [pendingSearchQuery])

  useEffect(() => {
    if (!searchQuery.trim()) {
      setSearchResults([])
      return
    }

    const performSearch = async () => {
      if (!window.api) return

      try {
        const results = await window.api.searchEntries(searchQuery)
        setSearchResults(results)
      } catch (error) {
        console.error('Search error', error)
      }
    }

    void performSearch()
  }, [searchQuery])

  useEffect(() => {
    if (!isSearchOpen) return

    const closeSearchOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsSearchOpen(false)
        setPendingSearchQuery('')
        setSearchQuery('')
      }
    }

    window.addEventListener('keydown', closeSearchOnEscape)
    return () => window.removeEventListener('keydown', closeSearchOnEscape)
  }, [isSearchOpen])

  useEffect(() => {
    const handleGlobalKeyboard = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'k') {
        event.preventDefault()
        setIsSearchOpen((prev) => !prev)
        if (!isSearchOpen) {
          setPendingSearchQuery('')
          setSearchQuery('')
        }
      }
    }

    window.addEventListener('keydown', handleGlobalKeyboard)
    return () => window.removeEventListener('keydown', handleGlobalKeyboard)
  }, [isSearchOpen])

  const handleSelectFolder = async () => {
    if (!window.api) return

    const path = await window.api.selectFolder()
    if (!path) {
      return
    }

    await window.api.setVaultPath(path)
    setVaultPath(path)
    setRecentVaultPaths(await window.api.getRecentVaultPaths())
    setCurrentEntry(null)
    setActiveSpace('my-space')
    await refreshData()
  }

  const handleSelectVaultPath = useCallback(async (nextVaultPath: string) => {
    if (!window.api || !nextVaultPath || nextVaultPath === vaultPath) {
      return
    }

    await window.api.setVaultPath(nextVaultPath)
    setVaultPath(nextVaultPath)
    setRecentVaultPaths(await window.api.getRecentVaultPaths())
    setCurrentEntry(null)
    setActiveSpace('my-space')
    setActiveScreen('notes')
    await refreshData()
  }, [refreshData, vaultPath])

  const findMySpaceEntry = useCallback((entryList: Entry[]) => {
    return entryList.find((entry) => entry.title.trim() === MY_SPACE_TITLE) ?? null
  }, [])

  const createEntry = useCallback((title: string) => {
    const newEntry: Entry = {
      id: uuidv4(),
      title,
      content_json: JSON.stringify({ type: 'doc', content: [{ type: 'paragraph' }] }),
      created_at: Date.now(),
      updated_at: Date.now(),
      folder_id: null,
      type_id: null,
      header_layout: null,
      header_props_json: '{}',
      schema_version: 1,
      deleted_at: null,
    }

    setEntries((prevEntries) => [newEntry, ...prevEntries])

    if (window.api) {
      void window.api.saveEntry(newEntry).then((saveResult) => {
        if (!saveResult.ok) {
          setEntries((prevEntries) => prevEntries.filter((entry) => entry.id !== newEntry.id))
          setCurrentEntry((prevEntry) => prevEntry?.id === newEntry.id ? null : prevEntry)
        }
      })
    }

    return newEntry
  }, [])

  const openMySpace = useCallback(async () => {
    setActiveScreen('notes')
    setActiveSpace('my-space')

    const existingEntry = findMySpaceEntry(entries)
    if (existingEntry) {
      setCurrentEntry(existingEntry)
      return
    }

    const mySpaceEntry = createEntry(MY_SPACE_TITLE)
    setCurrentEntry(mySpaceEntry)
  }, [createEntry, entries, findMySpaceEntry])

  useEffect(() => {
    if (isInitializing || !vaultPath || activeScreen !== 'notes') {
      return
    }

    if (activeSpace === 'my-space' && !currentEntry) {
      void openMySpace()
    }
  }, [activeScreen, activeSpace, currentEntry, isInitializing, openMySpace, vaultPath])

  const getUniqueDraftTitle = () => {
    const baseTitle = 'Новая заметка'
    const siblingTitles = new Set(entries.map((existingEntry) => existingEntry.title))

    if (!siblingTitles.has(baseTitle)) {
      return baseTitle
    }

    let suffix = 2
    while (siblingTitles.has(`${baseTitle} ${suffix}`)) {
      suffix += 1
    }

    return `${baseTitle} ${suffix}`
  }

  const createNewEntry = async () => {
    setActiveScreen('notes')

    const title = getUniqueDraftTitle()
    const newEntry = createEntry(title)
    setCurrentEntry(newEntry)
  }

  const handleSettingsChange = async (settingsPatch: Partial<CodeToolsSettings>) => {
    if (!window.api) return
    const nextSettings = await window.api.updateCodeToolsSettings(settingsPatch)
    setCodeToolsSettings(nextSettings)
  }

  const handleSaveNoteType = async (draft: Omit<NoteType, 'id' | 'created_at' | 'updated_at' | 'slug'> & { id?: string; slug?: string }) => {
    if (!window.api) return { ok: false as const }

    const now = Date.now()
    const noteType: NoteType = {
      id: draft.id ?? uuidv4(),
      name: draft.name,
      slug: normalizeSlug(draft.slug || draft.name),
      icon: draft.icon,
      color: draft.color,
      schema_json: draft.schema_json,
      header_template_json: draft.header_template_json,
      created_at: draft.id ? (noteTypes.find((item) => item.id === draft.id)?.created_at ?? now) : now,
      updated_at: now,
    }

    const result = await window.api.saveNoteType(noteType)
    if (result.ok) {
      await refreshData()
    }

    return result
  }

  const handleDeleteNoteType = async (noteTypeId: string) => {
    if (!window.api || isSystemType(noteTypeId)) return
    await window.api.deleteNoteType(noteTypeId)
    await refreshData()
  }

  const handleSave = async (entry: Entry): Promise<SaveEntryResult | null> => {
    if (!window.api) return null

    const persistEntry = async (entryToPersist: Entry): Promise<SaveEntryResult | null> => {
      latestSaveTimestampRef.current[entryToPersist.id] = entryToPersist.updated_at
      const saveResult = await window.api.saveEntry(entryToPersist)

      if (!saveResult.ok) {
        return saveResult
      }

      if (latestSaveTimestampRef.current[entryToPersist.id] !== entryToPersist.updated_at) {
        return saveResult
      }

      setEntries((prevEntries) => {
        return prevEntries.some((existingEntry) => existingEntry.id === entryToPersist.id)
          ? prevEntries.map((existingEntry) => existingEntry.id === entryToPersist.id ? entryToPersist : existingEntry)
          : [entryToPersist, ...prevEntries]
      })
      setCurrentEntry((prevEntry) => prevEntry?.id === entryToPersist.id ? entryToPersist : prevEntry)
      return saveResult
    }

    const coordinator = saveCoordinatorsRef.current[entry.id] ?? {
      inFlight: false,
      queued: null,
    }
    saveCoordinatorsRef.current[entry.id] = coordinator

    const runSaveLoop = async (nextEntry: Entry): Promise<SaveEntryResult | null> => {
      coordinator.inFlight = true

      try {
        const result = await persistEntry(nextEntry)
        const queuedRequest = coordinator.queued

        if (!queuedRequest) {
          coordinator.inFlight = false
          return result
        }

        coordinator.queued = null
        const queuedResult = await runSaveLoop(queuedRequest.entry)
        queuedRequest.waiters.forEach((waiter) => waiter.resolve(queuedResult))
        return queuedResult
      } catch (error) {
        const queuedRequest = coordinator.queued
        coordinator.queued = null
        coordinator.inFlight = false

        if (queuedRequest) {
          queuedRequest.waiters.forEach((waiter) => waiter.reject(error))
        }

        throw error
      } finally {
        if (!coordinator.queued) {
          coordinator.inFlight = false
          delete saveCoordinatorsRef.current[nextEntry.id]
        }
      }
    }

    if (!coordinator.inFlight) {
      return runSaveLoop(entry)
    }

    return new Promise<SaveEntryResult | null>((resolve, reject) => {
      if (coordinator.queued) {
        coordinator.queued.entry = entry
        coordinator.queued.waiters.push({ resolve, reject })
        return
      }

      coordinator.queued = {
        entry,
        waiters: [{ resolve, reject }],
      }
    })
  }

  const handleNavigate = async (entryId: string) => {
    if (!window.api) return
    const entry = await window.api.loadEntry(entryId)
    if (entry) {
      setActiveScreen('notes')
      setCurrentEntry(entry)
    }
  }

  const startSidebarAnimation = useCallback(() => {
    if (animTimerRef.current) {
      window.clearTimeout(animTimerRef.current)
    }
    isAnimatingRef.current = true
    setSidebarAnimating(true)
    animTimerRef.current = window.setTimeout(() => {
      isAnimatingRef.current = false
      setSidebarAnimating(false)
      animTimerRef.current = null
    }, 220)
  }, [])

  const handleResizeStart = useCallback((panel: 'vault' | 'widget') => (event: ReactMouseEvent) => {
    event.preventDefault()
    setActiveResizePanel(panel)
  }, [])

  const handleResizeMove = useCallback((event: MouseEvent) => {
    if (!activeResizePanel || isAnimatingRef.current) return

    if (resizeRef.current) {
      cancelAnimationFrame(resizeRef.current)
    }

    resizeRef.current = requestAnimationFrame(() => {
      if (isAnimatingRef.current) return

      if (activeResizePanel === 'vault') {
        const newWidth = event.clientX

        if (newWidth <= COLLAPSE_THRESHOLD) {
          if (!vaultSidebarCollapsed) {
            startSidebarAnimation()
            setVaultSidebarCollapsed(true)
          }
          return
        }

        if (vaultSidebarCollapsed) {
          startSidebarAnimation()
          setVaultSidebarCollapsed(false)
          setVaultSidebarWidth(MIN_VAULT_SIDEBAR_WIDTH)
          return
        }

        setVaultSidebarWidth(Math.max(MIN_VAULT_SIDEBAR_WIDTH, Math.min(MAX_VAULT_SIDEBAR_WIDTH, newWidth)))
        return
      }

      const vaultVisibleWidth = vaultSidebarCollapsed ? 0 : vaultSidebarWidth
      const newWidth = event.clientX - vaultVisibleWidth

      if (newWidth <= COLLAPSE_THRESHOLD) {
        if (!widgetSidebarCollapsed) {
          startSidebarAnimation()
          setWidgetSidebarCollapsed(true)
        }
        return
      }

      if (widgetSidebarCollapsed) {
        startSidebarAnimation()
        setWidgetSidebarCollapsed(false)
        setWidgetSidebarWidth(MIN_WIDGET_SIDEBAR_WIDTH)
        return
      }

      setWidgetSidebarWidth(Math.max(MIN_WIDGET_SIDEBAR_WIDTH, Math.min(MAX_WIDGET_SIDEBAR_WIDTH, newWidth)))
    })
  }, [activeResizePanel, vaultSidebarCollapsed, vaultSidebarWidth, widgetSidebarCollapsed, startSidebarAnimation])

  const handleResizeEnd = useCallback(() => {
    if (resizeRef.current) {
      cancelAnimationFrame(resizeRef.current)
      resizeRef.current = null
    }

    setActiveResizePanel(null)

    if (window.api) {
      void window.api.updateSidebarConfig({
        widget: { width: widgetSidebarWidth, collapsed: widgetSidebarCollapsed },
        vault: { width: vaultSidebarWidth, collapsed: vaultSidebarCollapsed },
      })
    }
  }, [vaultSidebarCollapsed, vaultSidebarWidth, widgetSidebarCollapsed, widgetSidebarWidth])

  useEffect(() => {
    if (!activeResizePanel) {
      return
    }

    window.addEventListener('mousemove', handleResizeMove)
    window.addEventListener('mouseup', handleResizeEnd)

    return () => {
      window.removeEventListener('mousemove', handleResizeMove)
      window.removeEventListener('mouseup', handleResizeEnd)
    }
  }, [activeResizePanel, handleResizeEnd, handleResizeMove])

  const handleToggleWidgetSidebar = useCallback(async () => {
    startSidebarAnimation()
    const nextCollapsed = !widgetSidebarCollapsed
    setWidgetSidebarCollapsed(nextCollapsed)
    if (window.api) {
      await window.api.updateSidebarConfig({ widget: { collapsed: nextCollapsed } })
    }
  }, [startSidebarAnimation, widgetSidebarCollapsed])

  const handleToggleVaultSidebar = useCallback(async () => {
    startSidebarAnimation()
    const nextCollapsed = !vaultSidebarCollapsed
    setVaultSidebarCollapsed(nextCollapsed)
    if (window.api) {
      await window.api.updateSidebarConfig({ vault: { collapsed: nextCollapsed } })
    }
  }, [startSidebarAnimation, vaultSidebarCollapsed])

  const handleSelectSpace = useCallback((spaceId: SpaceId) => {
    if (spaceId === 'my-space') {
      void openMySpace()
      return
    }

    setActiveScreen('notes')
    setActiveSpace(spaceId)
    setCurrentEntry(null)
  }, [openMySpace])

  useEffect(() => {
    const titlebarLeftSafeArea = (vaultSidebarCollapsed ? 0 : vaultSidebarWidth) + (widgetSidebarCollapsed ? 0 : widgetSidebarWidth)
    document.documentElement.style.setProperty('--titlebar-left-safe-area', `${titlebarLeftSafeArea}px`)

    return () => {
      document.documentElement.style.removeProperty('--titlebar-left-safe-area')
    }
  }, [vaultSidebarCollapsed, vaultSidebarWidth, widgetSidebarCollapsed, widgetSidebarWidth])

  if (isInitializing) {
    return (
      <div className="app-container loading">
        <Titlebar />
        Загрузка...
      </div>
    )
  }

  if (!vaultPath) {
    return (
      <div className="app-container setup-container">
        <Titlebar />
        <div className="setup-box">
          <h1>Добро пожаловать в Eden</h1>
          <p>Пожалуйста, выберите папку для хранения ваших заметок.</p>
          <button onClick={handleSelectFolder} className="setup-btn" type="button">Выбрать папку</button>
        </div>
      </div>
    )
  }

  return (
    <div className="app-container">
      <Titlebar />
      <SearchOverlay
        isOpen={isSearchOpen}
        query={pendingSearchQuery}
        results={searchResults}
        entries={entries}
        onQueryChange={setPendingSearchQuery}
        onClose={() => {
          setIsSearchOpen(false)
          setPendingSearchQuery('')
          setSearchQuery('')
        }}
        onResultSelect={async (entryId) => {
          const found = await window.api.loadEntry(entryId)
          if (found) {
            setActiveScreen('notes')
            setCurrentEntry(found)
            setPendingSearchQuery('')
            setSearchQuery('')
            setIsSearchOpen(false)
          }
        }}
      />
      <div className={`sidebar-layout ${activeResizePanel ? 'is-resizing' : ''}`} style={activeScreen === 'settings' ? { display: 'none' } : undefined}>
        <div
          className={`vault-sidebar-wrapper ${vaultSidebarCollapsed ? 'collapsed' : ''} ${sidebarAnimating ? 'sidebarAnimation' : ''} ${activeResizePanel === 'vault' ? 'is-resizing' : ''}`}
          style={{ width: vaultSidebarCollapsed ? 0 : vaultSidebarWidth }}
        >
          <VaultSidebar
            vaultPath={vaultPath}
            recentVaultPaths={recentVaultPaths}
            collapsed={vaultSidebarCollapsed}
            onToggleCollapsed={handleToggleVaultSidebar}
            onSelectVault={(nextVaultPath) => { void handleSelectVaultPath(nextVaultPath) }}
            onOpenVaultPicker={() => { void handleSelectFolder() }}
          />
          <div className="vault-sidebar-resize-handle" onMouseDown={handleResizeStart('vault')}>
            <div className="resize-handle-line" />
          </div>
        </div>
        <div
          className={`widget-sidebar-wrapper ${widgetSidebarCollapsed ? 'collapsed' : ''} ${sidebarAnimating ? 'sidebarAnimation' : ''} ${activeResizePanel === 'widget' ? 'is-resizing' : ''}`}
          style={{ width: widgetSidebarCollapsed ? 0 : widgetSidebarWidth }}
        >
          <WidgetSidebar
            isSearchOpen={isSearchOpen}
            isVaultSidebarCollapsed={vaultSidebarCollapsed}
            activeSpace={activeSpace}
            entries={entries}
            noteTypes={noteTypes}
            currentEntry={currentEntry}
            onToggleCollapse={handleToggleWidgetSidebar}
            onToggleVaultSidebar={handleToggleVaultSidebar}
            onSelectSpace={handleSelectSpace}
            onSearchToggle={() => setIsSearchOpen((prev) => !prev)}
            searchQuery={searchQuery}
            onCreateRootEntry={() => { void createNewEntry() }}
            onOpenEntry={(entryId) => { void handleNavigate(entryId) }}
            onOpenSettings={() => setActiveScreen('settings')}
          />
          <div className="widget-sidebar-resize-handle" onMouseDown={handleResizeStart('widget')}>
            <div className="resize-handle-line" />
          </div>
        </div>
      </div>
      <main className="app-main">
        {widgetSidebarCollapsed && (
          <button
            className="sidebar-head-icon withBackground sidebar-expand-btn"
            data-testid="sidebar-toggle-external"
            onClick={() => { void handleToggleWidgetSidebar() }}
            title="Открыть виджеты"
            type="button"
          >
            <span aria-hidden="true" className="anytype-icon toggleWidget" />
          </button>
        )}
        {activeScreen === 'settings' ? (
          <SettingsPage
            settings={codeToolsSettings}
            vaultPath={vaultPath}
            noteTypes={noteTypes}
            onBack={() => setActiveScreen('notes')}
            onSelectVault={handleSelectFolder}
            onExportMarkdownVault={() => window.api.exportMarkdownVault()}
            onSettingsChange={handleSettingsChange}
            onNoteTypeSave={handleSaveNoteType}
            onNoteTypeDelete={handleDeleteNoteType}
            onRefreshData={refreshData}
          />
        ) : currentEntry ? (
          <Editor
            key={currentEntry.id}
            entry={currentEntry}
            allEntries={entries}
            noteTypes={noteTypes}
            codeToolsSettings={codeToolsSettings}
            onSave={handleSave}
            onNavigate={handleNavigate}
          />
        ) : (
          <SpacesView
            activeSpace={activeSpace}
            entries={entries}
            noteTypes={noteTypes}
            sortMode={sortMode}
            onSortModeChange={setSortMode}
            onCreateEntry={() => { void createNewEntry() }}
            onOpenEntry={(entryId) => { void handleNavigate(entryId) }}
          />
        )}
      </main>
    </div>
  )
}
