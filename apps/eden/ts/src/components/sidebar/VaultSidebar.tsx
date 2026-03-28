import { useMemo, useState } from "react";

interface VaultSidebarProps {
  vaultPath: string | null;
  recentVaultPaths: string[];
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onSelectVault: (vaultPath: string) => void;
  onOpenVaultPicker: () => void;
}

function getVaultLabel(vaultPath: string) {
  const parts = vaultPath.split(/[/\\]/);
  return parts[parts.length - 1] || vaultPath;
}

function getVaultInitial(vaultPath: string) {
  const label = getVaultLabel(vaultPath).trim();
  return label ? label[0]!.toUpperCase() : "E";
}

function AnytypeIcon({
  name,
}: {
  name: "gallery" | "plus" | "settings" | "toggleVault";
}) {
  return <span aria-hidden="true" className={`anytype-icon ${name}`} />;
}

export default function VaultSidebar({
  vaultPath,
  recentVaultPaths,
  onToggleCollapsed,
  onSelectVault,
  onOpenVaultPicker,
}: VaultSidebarProps) {
  const [filterQuery, setFilterQuery] = useState("");

  const visibleVaults = useMemo(() => {
    if (!vaultPath) {
      return [];
    }

    const merged = [
      vaultPath,
      ...recentVaultPaths.filter((item) => item !== vaultPath),
    ];
    return merged.slice(0, 12);
  }, [recentVaultPaths, vaultPath]);

  const filteredVaults = useMemo(() => {
    const normalizedQuery = filterQuery.trim().toLowerCase();
    if (!normalizedQuery) {
      return visibleVaults;
    }

    return visibleVaults.filter((item) =>
      getVaultLabel(item).toLowerCase().includes(normalizedQuery),
    );
  }, [filterQuery, visibleVaults]);

  return (
    <div className="vault-sidebar">
      <aside
        className="vault-browser sidebarPage pageVault pageVaultSingle"
        data-testid="vault-switcher-menu"
      >
        <div className="head">
          <div className="side left">
            <div className="name">Хранилища</div>
          </div>
          <div className="side right">
            <button
              className="sidebar-head-icon withBackground"
              onClick={onOpenVaultPicker}
              title="Добавить хранилище"
              type="button"
            >
              <AnytypeIcon name="plus" />
            </button>
            <button
              className="sidebar-head-icon withBackground"
              onClick={onToggleCollapsed}
              title="Скрыть хранилища"
              type="button"
            >
              <AnytypeIcon name="toggleVault" />
            </button>
          </div>
        </div>

        <div className="filterWrapper">
          <input
            className="vault-browser-search"
            onChange={(event) => setFilterQuery(event.target.value)}
            placeholder="Поиск"
            type="text"
            value={filterQuery}
          />
        </div>

        <div className="body">
          <div className="scrollArea vault-browser-list">
            {filteredVaults.map((item) => (
              <button
                key={item}
                className={`item vault-browser-item ${item === vaultPath ? "active" : ""}`}
                data-testid={`vault-item-${getVaultLabel(item)}`}
                onClick={() => onSelectVault(item)}
                type="button"
              >
                <span className="iconWrap">
                  <span className="vault-avatar">{getVaultInitial(item)}</span>
                </span>
                <span className="info">
                  <span className="nameWrapper">
                    <span className="name">{getVaultLabel(item)}</span>
                  </span>
                  <span className="messageWrapper">
                    <span className="label lastMessage">Локальная папка</span>
                  </span>
                </span>
              </button>
            ))}

            {filteredVaults.length === 0 && (
              <div className="vault-browser-empty">Ничего не найдено</div>
            )}
          </div>
        </div>

        <div className="bottom">
          <div className="grad" />
          <div className="sides">
            <div className="side left">
              <button
                className="appSettings"
                onClick={vaultPath ? () => onSelectVault(vaultPath) : undefined}
                type="button"
              >
                <span className="iconWrap">
                  <span className="vault-avatar">
                    {vaultPath ? getVaultInitial(vaultPath) : "E"}
                  </span>
                </span>
                <span className="name">
                  {vaultPath ? getVaultLabel(vaultPath) : "Eden"}
                </span>
              </button>
            </div>
          </div>
        </div>
      </aside>
    </div>
  );
}
