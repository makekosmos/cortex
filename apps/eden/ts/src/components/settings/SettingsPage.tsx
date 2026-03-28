import { useState } from 'react'
import GeneralSettings from './GeneralSettings'
import TrashSettings from './TrashSettings'
import StorageSettings from './StorageSettings'
import ObjectTypesSettings from './ObjectTypesSettings'
import ConnectedAppsSettings from './ConnectedAppsSettings'
import './SettingsPage.css'

export type SettingsTab = 'general' | 'trash' | 'storage' | 'object-types' | 'connected-apps'

interface SettingsPageProps {
  settings: CodeToolsSettings | null;
  vaultPath: string;
  noteTypes: NoteType[];
  onBack: () => void;
  onSelectVault: () => Promise<void>;
  onExportMarkdownVault: () => Promise<ExportMarkdownVaultResult | null>;
  onSettingsChange: (settings: Partial<CodeToolsSettings>) => Promise<void>;
  onNoteTypeSave: (noteType: Omit<NoteType, 'id' | 'created_at' | 'updated_at' | 'slug'> & { id?: string; slug?: string }) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
  onRefreshData: () => Promise<void>;
}

const navSections = [
  {
    title: 'Настройки',
    items: [
      { id: 'general' as SettingsTab, label: 'Общие', icon: 'settings-space' },
      { id: 'trash' as SettingsTab, label: 'Корзина', icon: 'settings-bin' },
      { id: 'storage' as SettingsTab, label: 'Хранилище', icon: 'settings-storage' },
      { id: 'connected-apps' as SettingsTab, label: 'Связанные программы', icon: 'settings-space' },
    ],
  },
  {
    title: 'Модель содержимого',
    items: [
      { id: 'object-types' as SettingsTab, label: 'Типы объектов', icon: 'settings-type' },
    ],
  },
]

export default function SettingsPage({
  settings,
  vaultPath,
  noteTypes,
  onBack,
  onSelectVault,
  onExportMarkdownVault,
  onSettingsChange,
  onNoteTypeSave,
  onNoteTypeDelete,
  onRefreshData,
}: SettingsPageProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('general')

  return (
    <div className="settings-page">
      <nav className="settings-nav">
        <div className="settings-nav-head">
          <button className="settings-nav-back" onClick={onBack} type="button">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <path d="M12 4L6 10L12 16" />
            </svg>
          </button>
          <span className="settings-nav-head-title">Настройки</span>
        </div>
        {navSections.map((section) => (
          <div key={section.title} className="settings-nav-group">
            <div className="settings-nav-section">{section.title}</div>
            {section.items.map((item) => (
              <button
                key={item.id}
                className={`settings-nav-item ${activeTab === item.id ? 'active' : ''}`}
                onClick={() => setActiveTab(item.id)}
                type="button"
              >
                <span className={`settings-nav-icon icon ${item.icon}`} />
                <span className="settings-nav-label">{item.label}</span>
              </button>
            ))}
          </div>
        ))}
      </nav>
      <div className="settings-content">
        {activeTab === 'general' && (
          <GeneralSettings
            settings={settings}
            vaultPath={vaultPath}
            onSelectVault={onSelectVault}
            onExportMarkdownVault={onExportMarkdownVault}
            onSettingsChange={onSettingsChange}
          />
        )}
        {activeTab === 'trash' && (
          <TrashSettings onRefreshData={onRefreshData} />
        )}
        {activeTab === 'storage' && (
          <StorageSettings vaultPath={vaultPath} />
        )}
        {activeTab === 'object-types' && (
          <ObjectTypesSettings
            noteTypes={noteTypes}
            onNoteTypeSave={onNoteTypeSave}
            onNoteTypeDelete={onNoteTypeDelete}
          />
        )}
        {activeTab === 'connected-apps' && (
          <ConnectedAppsSettings onRefreshData={onRefreshData} />
        )}
      </div>
    </div>
  )
}
