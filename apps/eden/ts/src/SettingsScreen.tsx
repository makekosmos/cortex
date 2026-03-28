import './SettingsScreen.css'

interface SettingsScreenProps {
  settings: CodeToolsSettings | null;
  vaultPath: string;
  onBack: () => void;
  onOpenNoteTypes: () => void;
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
  { value: 'myagkiy', label: 'Мягкий', description: 'Подсказывает только важное, не перегружает предупреждениями.' },
  { value: 'balans', label: 'Баланс', description: 'Оптимальный режим для ежедневной работы.' },
  { value: 'strogiy', label: 'Строгий', description: 'Максимальная проверка качества и единообразия кода.' },
]

export default function SettingsScreen({ settings, vaultPath, onBack, onOpenNoteTypes, onSelectVault, onExportMarkdownVault, onSettingsChange }: SettingsScreenProps) {
  const normalizedSettings = settings ?? defaultSettings

  const handleExport = async () => {
    const result = await onExportMarkdownVault()
    if (!result) {
      return
    }

    if (result.ok) {
      window.alert(`Экспортировано заметок: ${result.exportedCount}\nПапка: ${result.outputDir}`)
      return
    }

    window.alert('Не удалось экспортировать markdown')
  }

  return (
    <section className="settings-screen">
      <div className="settings-rail">
        <div className="settings-header">
          <div>
            <p className="settings-kicker">Настройки</p>
            <h1 className="settings-title">Проверка, формат и хранилище</h1>
            <p className="settings-subtitle">Простой режим: выбери стиль проверки, когда запускать линтер и где хранить заметки.</p>
          </div>
          <button className="settings-back-btn" onClick={onBack} type="button">К заметкам</button>
        </div>

        <div className="settings-grid">
          <section className="settings-card">
            <div className="settings-card-header">
              <h2>Хранилище</h2>
              <p>Папка, где Eden хранит базу заметок. Markdown теперь экспортируется вручную.</p>
            </div>
            <div className="settings-field-stack">
              <label className="settings-field">
                <span>Текущая папка</span>
                <code className="settings-path">{vaultPath}</code>
              </label>
              <button className="settings-primary-btn" onClick={() => void onSelectVault()} type="button">Сменить папку</button>
              <button className="settings-primary-btn" onClick={() => void handleExport()} type="button">Экспортировать vault в Markdown</button>
            </div>
          </section>

          <section className="settings-card">
            <div className="settings-card-header">
              <h2>Типы заметок</h2>
              <p>Создавай собственные типы с полями и визуальными верхушками на отдельной странице.</p>
            </div>
            <div className="settings-field-stack">
              <button className="settings-primary-btn" onClick={onOpenNoteTypes} type="button">Открыть типы заметок</button>
            </div>
          </section>

          <section className="settings-card settings-card-wide">
            <div className="settings-card-header">
              <h2>Режим проверки</h2>
              <p>Общий пресет для `oxlint` и `oxfmt`, без технических полей.</p>
            </div>
            <div className="settings-field-grid">
              {presetOptions.map((option) => (
                <label className="settings-checkbox" key={option.value}>
                  <input checked={normalizedSettings.preset === option.value} onChange={() => void onSettingsChange({ preset: option.value })} name="preset" type="radio" />
                  <span>{option.label} - {option.description}</span>
                </label>
              ))}
            </div>
          </section>

          <section className="settings-card settings-card-wide">
            <div className="settings-card-header">
              <h2>Когда запускать проверку</h2>
              <p>Выберите удобный момент: сразу после сохранения или автоматически в паузе.</p>
            </div>
            <div className="settings-field-grid">
              <label className="settings-checkbox">
                <input checked={normalizedSettings.lintTrigger === 'on_save'} onChange={() => void onSettingsChange({ lintTrigger: 'on_save' })} name="lintTrigger" type="radio" />
                <span>При сохранении заметки</span>
              </label>
              <label className="settings-checkbox">
                <input checked={normalizedSettings.lintTrigger === 'on_idle'} onChange={() => void onSettingsChange({ lintTrigger: 'on_idle' })} name="lintTrigger" type="radio" />
                <span>Через 1 секунду после паузы в наборе</span>
              </label>
              <label className="settings-checkbox">
                <input checked={normalizedSettings.formatOnSave} onChange={(event) => void onSettingsChange({ formatOnSave: event.target.checked })} type="checkbox" />
                <span>Автоформат кода при сохранении</span>
              </label>
            </div>
          </section>
        </div>
      </div>
    </section>
  )
}
