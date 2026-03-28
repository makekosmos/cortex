import { useEffect, useMemo, useRef, useState } from "react";
import type { SpaceId } from "./SpaceRail";

interface WidgetSidebarProps {
  vaultPath?: string | null;
  isSearchOpen?: boolean;
  isVaultSidebarCollapsed: boolean;
  activeSpace: SpaceId;
  entries: Entry[];
  noteTypes: NoteType[];
  currentEntry: Entry | null;
  onToggleCollapse: () => void;
  onToggleVaultSidebar: () => void;
  onSelectSpace: (spaceId: SpaceId) => void;
  onSearchToggle?: () => void;
  searchQuery: string;
  onCreateRootEntry: () => void;
  onOpenEntry: (entryId: string) => void;
  onOpenSettings: () => void;
}

type SectionId = "pinned" | "recent" | `type-${string}`;
type AnytypeIconName =
  | "clock"
  | "collapseArrow"
  | "create"
  | "document"
  | "expand"
  | "more"
  | "plus"
  | "search"
  | "settings"
  | "toggleVault"
  | "toggleWidget";

const spaceTitles: Record<Exclude<SpaceId, "my-space">, string> = {
  "all-objects": "Все объекты",
  "all-notes": "Заметки",
  "all-properties": "Все свойства",
  "diary": "Дневник",
};

const labels = {
  pinned: "Закреплённые",
  recent: "Недавно изменённые",
  objects: "Объекты",
  mySpace: "Моё пространство",
  search: "Поиск",
  create: "Создать",
  newNote: "Новая заметка",
  settings: "Настройки",
  openVaultSidebar: "Показать хранилища",
  hideVaultSidebar: "Скрыть хранилища",
  collapseWidgets: "Скрыть виджеты",
  recentlyOpened: "Недавно открытые",
};

function AnytypeIcon({ name }: { name: AnytypeIconName }) {
  return <span aria-hidden="true" className={`anytype-icon ${name}`} />;
}

function getEntryLabel(entry: Entry) {
  return entry.title.trim() || "Без названия";
}

function getEntryCaption(entry: Entry, noteTypes: NoteType[]) {
  if (entry.type_id) {
    const noteType = noteTypes.find((nt) => nt.id === entry.type_id);
    if (noteType) return noteType.name;
  }
  return "Страница";
}

function ObjectIcon({ entry, noteTypes, size = 20 }: { entry: Entry; noteTypes: NoteType[]; size?: number }) {
  if (entry.type_id) {
    const noteType = noteTypes.find((nt) => nt.id === entry.type_id);
    if (noteType) {
      return (
        <span className="objectIcon objectIcon-type" style={{ width: size, height: size, color: noteType.color ?? undefined }}>
          {noteType.icon ?? "T"}
        </span>
      );
    }
  }
  return (
    <img
      className="objectIcon objectIcon-page"
      src="/anytype/icon/object/page.svg"
      alt=""
      width={size}
      height={size}
      draggable={false}
    />
  );
}

export default function WidgetSidebar({
  isSearchOpen,
  isVaultSidebarCollapsed,
  activeSpace,
  entries,
  noteTypes,
  currentEntry,
  onToggleCollapse,
  onToggleVaultSidebar,
  onSelectSpace,
  onSearchToggle,
  searchQuery,
  onCreateRootEntry,
  onOpenEntry,
  onOpenSettings,
}: WidgetSidebarProps) {
  const [collapsedSections, setCollapsedSections] = useState<
    Record<string, boolean>
  >({
    pinned: false,
    recent: false,
  });
  const [isRecentlyOpenedOpen, setIsRecentlyOpenedOpen] = useState(false);
  const recentlyOpenedRef = useRef<HTMLDivElement>(null);
  const recentlyOpenedBtnRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!isRecentlyOpenedOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (
        recentlyOpenedRef.current?.contains(event.target as Node) ||
        recentlyOpenedBtnRef.current?.contains(event.target as Node)
      ) {
        return;
      }
      setIsRecentlyOpenedOpen(false);
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setIsRecentlyOpenedOpen(false);
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [isRecentlyOpenedOpen]);

  const recentEntries = useMemo(
    () => [...entries].sort((a, b) => b.updated_at - a.updated_at).slice(0, 6),
    [entries],
  );

  const recentlyOpenedEntries = useMemo(
    () => [...entries].sort((a, b) => b.updated_at - a.updated_at).slice(0, 10),
    [entries],
  );

  const typedSections = useMemo(() => {
    return noteTypes
      .map((noteType) => ({
        id: `type-${noteType.id}` as SectionId,
        noteType,
        entries: entries
          .filter((entry) => entry.type_id === noteType.id)
          .sort((a, b) => b.updated_at - a.updated_at)
          .slice(0, 4),
      }))
      .filter((section) => section.entries.length > 0)
      .slice(0, 4);
  }, [entries, noteTypes]);

  const pinnedItems = useMemo(() => {
    return [
      {
        id: "my-space",
        iconClassName: "person-badge",
        label: labels.mySpace,
        active: activeSpace === "my-space",
        testId: "widget-link-my-space",
        onClick: () => onSelectSpace("my-space"),
      },
    ];
  }, [activeSpace, onSelectSpace]);

  const objectItems = useMemo(() => {
    return [
      {
        id: "all-objects",
        iconClassName: "collection-badge",
        label: spaceTitles["all-objects"],
        active: activeSpace === "all-objects",
        testId: "widget-link-all-objects",
        onClick: () => onSelectSpace("all-objects"),
      },
      {
        id: "all-properties",
        iconClassName: "collection-badge",
        label: spaceTitles["all-properties"],
        active: activeSpace === "all-properties",
        testId: "widget-link-all-properties",
        onClick: () => onSelectSpace("all-properties"),
      },
      {
        id: "all-notes",
        iconClassName: "document-badge",
        label: spaceTitles["all-notes"],
        active: activeSpace === "all-notes",
        testId: "widget-link-all-notes",
        onClick: () => onSelectSpace("all-notes"),
      },
      {
        id: "diary",
        iconClassName: "document-badge",
        label: spaceTitles["diary"],
        active: activeSpace === "diary",
        testId: "widget-link-diary",
        onClick: () => onSelectSpace("diary"),
      },
    ];
  }, [activeSpace, onSelectSpace]);

  const toggleSection = (sectionId: SectionId) => {
    setCollapsedSections((current) => ({
      ...current,
      [sectionId]: !current[sectionId],
    }));
  };

  return (
    <aside className="widget-sidebar sidebarPage pageWidget">
      <div className="head">
        <div className="side left">
          {isVaultSidebarCollapsed && (
            <button
              className="sidebar-head-icon withBackground"
              onClick={onToggleVaultSidebar}
              title={labels.openVaultSidebar}
              type="button"
            >
              <AnytypeIcon name="toggleVault" />
            </button>
          )}
          <button
            className="sidebar-head-icon withBackground"
            data-testid="sidebar-toggle"
            onClick={onToggleCollapse}
            title={labels.collapseWidgets}
            type="button"
          >
            <AnytypeIcon name="toggleWidget" />
          </button>
        </div>

        <div className="side right" style={{ position: "relative" }}>
          <button
            ref={recentlyOpenedBtnRef}
            className={`sidebar-head-icon withBackground ${isRecentlyOpenedOpen ? "active" : ""}`}
            onClick={() => setIsRecentlyOpenedOpen((prev) => !prev)}
            title={labels.recentlyOpened}
            type="button"
          >
            <AnytypeIcon name="clock" />
          </button>

          {isRecentlyOpenedOpen && (
            <div ref={recentlyOpenedRef} className="recently-opened-menu">
              <div className="recently-opened-section-name">
                <span>Недавно открытые</span>
              </div>
              <div className="recently-opened-items">
                {recentlyOpenedEntries.map((entry) => (
                    <button
                      key={entry.id}
                      className={`recently-opened-item ${currentEntry?.id === entry.id ? "active" : ""}`}
                      onClick={() => {
                        onOpenEntry(entry.id);
                        setIsRecentlyOpenedOpen(false);
                      }}
                      type="button"
                    >
                      <span className="recently-opened-icon">
                        <ObjectIcon entry={entry} noteTypes={noteTypes} size={18} />
                      </span>
                      <span className="recently-opened-name">
                        {getEntryLabel(entry)}
                      </span>
                      <span className="recently-opened-caption">
                        {getEntryCaption(entry, noteTypes)}
                      </span>
                    </button>
                ))}
                {recentlyOpenedEntries.length === 0 && (
                  <div className="recently-opened-empty">Нет недавних записей</div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="body widget-sidebar-body">
        <div className="content">
          <section className="section widget-space-card">
            <div className="items">
              <button
                className="item widget-primary-item"
                onClick={onCreateRootEntry}
                type="button"
              >
                <span className="itemIcon">
                  <AnytypeIcon name="create" />
                </span>
                <span className="value">{labels.create}</span>
              </button>

              <button
                className={`item widget-primary-item ${isSearchOpen || searchQuery ? "active" : ""}`}
                data-testid="widget-link-search"
                onClick={() => onSearchToggle?.()}
                type="button"
              >
                <span className="itemIcon">
                  <AnytypeIcon name="search" />
                </span>
                <span className="value">{labels.search}</span>
              </button>
            </div>
          </section>

          <div
            className={`widgetSection ${collapsedSections.pinned ? "" : "isOpen"}`}
          >
            <div className="nameWrap">
              <div
                className="name"
                onClick={() => toggleSection("pinned")}
              >
                <AnytypeIcon name="collapseArrow" />
                {labels.pinned}
              </div>
              <div className="buttons">
                <button
                  className="sidebar-section-icon"
                  onClick={onCreateRootEntry}
                  title={labels.newNote}
                  type="button"
                >
                  <AnytypeIcon name="plus" />
                </button>
              </div>
            </div>

            <div className="itemsWrap">
              <div className="items">
                {pinnedItems.map((item) => (
                  <button
                    key={item.id}
                    className={`item widget-nav-item ${item.active ? "active" : ""}`}
                    data-testid={item.testId}
                    onClick={item.onClick}
                    type="button"
                  >
                    <span className="itemIcon">
                      <span className={`itemBadge ${item.iconClassName}`} />
                    </span>
                    <span className="value">{item.label}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div
            className={`widgetSection ${collapsedSections.recent ? "" : "isOpen"}`}
          >
            <div className="nameWrap">
              <div
                className="name"
                onClick={() => toggleSection("recent")}
              >
                <AnytypeIcon name="collapseArrow" />
                {labels.recent}
              </div>
            </div>

            <div className="itemsWrap">
              <div className="items">
                {recentEntries.map((entry) => (
                  <button
                    key={entry.id}
                    className={`item widget-nav-item ${currentEntry?.id === entry.id ? "active" : ""}`}
                    onClick={() => onOpenEntry(entry.id)}
                    type="button"
                  >
                    <span className="itemIcon">
                      <ObjectIcon entry={entry} noteTypes={noteTypes} size={18} />
                    </span>
                    <span className="value">{getEntryLabel(entry)}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="widgetSection isOpen">
            <div className="nameWrap">
              <div
                className="name"
                onClick={() => onSelectSpace("all-objects")}
              >
                <AnytypeIcon name="collapseArrow" />
                {labels.objects}
              </div>
            </div>

            <div className="itemsWrap">
              <div className="items">
                {objectItems.map((item) => (
                  <button
                    key={item.id}
                    className={`item widget-nav-item ${item.active ? "active" : ""}`}
                    data-testid={item.testId}
                    onClick={item.onClick}
                    type="button"
                  >
                    <span className="itemIcon">
                      <span className={`itemBadge ${item.iconClassName}`} />
                    </span>
                    <span className="value">{item.label}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>

          {typedSections.map(({ id, noteType, entries: typedEntries }) => (
            <div
              className={`widgetSection ${collapsedSections[id] ? "" : "isOpen"}`}
              key={noteType.id}
            >
              <div className="nameWrap">
                <div
                  className="name"
                  onClick={() => toggleSection(id)}
                >
                  <AnytypeIcon name="collapseArrow" />
                  {noteType.name}
                </div>
              </div>

              <div className="itemsWrap">
                <div className="items">
                  {typedEntries.map((entry) => (
                    <button
                      key={entry.id}
                      className={`item widget-nav-item ${currentEntry?.id === entry.id ? "active" : ""}`}
                      onClick={() => onOpenEntry(entry.id)}
                      type="button"
                    >
                      <span className="itemIcon">
                        <ObjectIcon entry={entry} noteTypes={noteTypes} size={18} />
                      </span>
                      <span className="value">{getEntryLabel(entry)}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="bottom widget-sidebar-bottom">
        <div className="grad" />
        <div className="sides">
          <div className="side left">
            <button
              className="widgetSettings"
              data-testid="open-settings-btn"
              onClick={onOpenSettings}
              title={labels.settings}
              type="button"
            >
              <AnytypeIcon name="settings" />
            </button>
          </div>
        </div>
      </div>
    </aside>
  );
}
