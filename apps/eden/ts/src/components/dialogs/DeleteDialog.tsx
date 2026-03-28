interface DeleteDialogProps {
  target: { kind: 'entry' | 'folder'; id: string; title: string } | null;
  onCancel: () => void;
  onConfirm: (target: { kind: 'entry' | 'folder'; id: string; title: string }) => void;
}

export default function DeleteDialog({ target, onCancel, onConfirm }: DeleteDialogProps) {
  if (!target) {
    return null
  }

  return (
    <div className="dialog-backdrop" data-testid="delete-dialog" onClick={onCancel}>
      <div className="dialog-card" onClick={(event) => event.stopPropagation()}>
        <div className="dialog-header">
          <div>
            <p className="dialog-kicker">Удаление</p>
            <h2 className="dialog-title">
              {target.kind === 'entry' ? 'Удалить заметку?' : 'Удалить папку?'}
            </h2>
          </div>
          <button className="dialog-close-btn" onClick={onCancel} type="button">
            ×
          </button>
        </div>
        <div className="dialog-field">
          <p>
            Вы уверены, что хотите удалить {target.kind === 'entry' ? 'заметку' : 'папку'} <strong>{target.title}</strong>?
            {target.kind === 'folder' && ' Удаление доступно только для пустой папки без вложенных папок и заметок.'}
          </p>
        </div>
        <div className="dialog-actions">
          <button className="dialog-secondary-btn" data-testid="delete-dialog-cancel" onClick={onCancel} type="button">
            Отмена
          </button>
          <button
            className="dialog-primary-btn dialog-primary-btn-danger"
            data-testid="delete-dialog-submit"
            onClick={() => onConfirm(target)}
            type="button"
          >
            Удалить
          </button>
        </div>
      </div>
    </div>
  )
}
