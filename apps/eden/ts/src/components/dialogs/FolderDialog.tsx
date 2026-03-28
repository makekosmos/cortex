import { RefObject } from 'react'

interface FolderDialogProps {
  isOpen: boolean;
  isCreating: boolean;
  name: string;
  error: string | null;
  inputRef: RefObject<HTMLInputElement>;
  onNameChange: (value: string) => void;
  onClose: () => void;
  onSubmit: () => void;
}

export default function FolderDialog({
  isOpen,
  isCreating,
  name,
  error,
  inputRef,
  onNameChange,
  onClose,
  onSubmit,
}: FolderDialogProps) {
  if (!isOpen) {
    return null
  }

  return (
    <div className="dialog-backdrop" data-testid="folder-dialog" onClick={onClose}>
      <div className="dialog-card" onClick={(event) => event.stopPropagation()}>
        <div className="dialog-header">
          <div>
            <p className="dialog-kicker">Новая папка</p>
            <h2 className="dialog-title">Создать папку</h2>
          </div>
          <button className="dialog-close-btn" onClick={onClose} type="button">
            ×
          </button>
        </div>
        <label className="dialog-field">
          <span>Название папки</span>
          <input
            ref={inputRef}
            className="dialog-input"
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder="Например, Проекты"
            type="text"
          />
        </label>
        {error && <div className="dialog-error">{error}</div>}
        <div className="dialog-actions">
          <button className="dialog-secondary-btn" data-testid="folder-dialog-cancel" onClick={onClose} type="button">
            Отмена
          </button>
          <button className="dialog-primary-btn" data-testid="folder-dialog-submit" disabled={isCreating} onClick={onSubmit} type="button" title="Создать папку">
            {isCreating ? 'Создание...' : 'Создать папку'}
          </button>
        </div>
      </div>
    </div>
  )
}
