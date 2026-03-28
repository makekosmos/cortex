interface GeneralSettingsProps {
  settings: CodeToolsSettings | null;
  vaultPath: string;
  onSelectVault: () => Promise<void>;
  onExportMarkdownVault: () => Promise<ExportMarkdownVaultResult | null>;
  onSettingsChange: (settings: Partial<CodeToolsSettings>) => Promise<void>;
}

const defaultSettings: CodeToolsSettings = {
  formatOnSave: true,
  preset: 'balans',
  lintTrigger: 'on_idle',
}

const presetOptions: Array<{ value: CodeToolsSettings['preset']; label: string; description: string }> = [
  { value: 'myagkiy', label: 'Мягкий', description: 'Подсказывает только важное.' },
  { value: 'balans', label: 'Баланс', description: 'Оптимальный режим.' },
  { value: 'strogiy', label: 'Строгий', description: 'Максимальная проверка.' },
]

export default function GeneralSettings({ settings, vaultPath, onSelectVault, onExportMarkdownVault, onSettingsChange }: GeneralSettingsProps) {
  const s = settings ?? defaultSettings

  const handleExport = async () => {
    const result = await onExportMarkdownVault()
    if (result?.ok) {
      window.alert(`Экспортировано: ${result.exportedCount}\nПапка: ${result.outputDir}`)
    }
  }

  return (
    <div className="settings-tab">
      <h1 className="settings-tab-title">Общие</h1>
      <p className="settings-tab-subtitle">Основные настройки пространства</p>

      <div className="settings-sections">
        <section className="settings-section">
          <div className="settings-section-header">
            <h2>Хранилище</h2>
          </div>
          <div className="settings-section-body">
            <div className="settings-row">
              <div className="settings-row-left">
                <div className="settings-row-title">Текущая папка</div>
                <div className="settings-row-desc">{vaultPath}</div>
              </div>
            </div>
            <div className="settings-row-actions">
              <button className="settings-btn-secondary" onClick={() => void onSelectVault()} type="button">Сменить папку</button>
              <button className="settings-btn-secondary" onClick={() => void handleExport()} type="button">Экспорт в Markdown</button>
            </div>
          </div>
        </section>

        <section className="settings-section">
          <div className="settings-section-header">
            <h2>Проверка кода</h2>
          </div>
          <div className="settings-section-body">
            <div className="settings-row">
              <div className="settings-row-left">
                <div className="settings-row-title">Режим проверки</div>
              </div>
              <div className="settings-row-right">
                <select
                  className="settings-select-inline"
                  value={s.preset}
                  onChange={(e) => void onSettingsChange({ preset: e.target.value as CodeToolsSettings['preset'] })}
                >
                  {presetOptions.map((o) => (
                    <option key={o.value} value={o.value}>{o.label} — {o.description}</option>
                  ))}
                </select>
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-left">
                <div className="settings-row-title">Когда запускать</div>
              </div>
              <div className="settings-row-right">
                <select
                  className="settings-select-inline"
                  value={s.lintTrigger}
                  onChange={(e) => void onSettingsChange({ lintTrigger: e.target.value as CodeToolsSettings['lintTrigger'] })}
                >
                  <option value="on_save">При сохранении</option>
                  <option value="on_idle">После паузы в наборе</option>
                </select>
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-left">
                <div className="settings-row-title">Автоформат при сохранении</div>
              </div>
              <div className="settings-row-right">
                <input
                  type="checkbox"
                  checked={s.formatOnSave}
                  onChange={(e) => void onSettingsChange({ formatOnSave: e.target.checked })}
                />
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
