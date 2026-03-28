import { parseHeaderTemplate, parseNoteTypeDefinition } from '@/lib/typedNotes'

interface TypedHeaderProps {
  activeNoteType: NoteType | null;
  title: string;
  headerProps: Record<string, unknown>;
  validationError: string | null;
  onHeaderPropChange: (fieldId: string, value: unknown) => void;
}

export default function TypedHeader({
  activeNoteType,
  title,
  headerProps,
  validationError,
  onHeaderPropChange,
}: TypedHeaderProps) {
  const noteTypeDefinition = activeNoteType ? parseNoteTypeDefinition(activeNoteType.schema_json) : null
  const headerTemplate = activeNoteType ? parseHeaderTemplate(activeNoteType.header_template_json) : null
  const primaryFieldIds = headerTemplate?.primaryFieldIds ?? []
  const secondaryFieldIds = headerTemplate?.secondaryFieldIds ?? []
  const imageFieldId = headerTemplate?.imageFieldId ?? null
  const primaryText = primaryFieldIds
    .map(fieldId => String(headerProps[fieldId] ?? '').trim())
    .filter(Boolean)
    .join(' ')
  const secondaryText = secondaryFieldIds
    .map(fieldId => String(headerProps[fieldId] ?? '').trim())
    .filter(Boolean)
    .join(' · ')
  const imageSrc = imageFieldId ? String(headerProps[imageFieldId] ?? '').trim() : ''

  return (
    <div className="typed-note-shell">
      {activeNoteType && noteTypeDefinition && (
        <div className={`typed-note-hero typed-note-hero-${headerTemplate?.kind ?? 'default'}`} data-testid="typed-note-header">
          {headerTemplate?.kind === 'centered_profile' && (
            <>
              <div className="typed-note-avatar-wrap">
                {imageSrc ? <img className="typed-note-avatar" data-testid="typed-note-avatar" src={imageSrc} alt={title || activeNoteType.name} /> : <div className="typed-note-avatar typed-note-avatar-placeholder">{activeNoteType.icon ?? '✦'}</div>}
              </div>
              <div className="typed-note-hero-text">
                <h2 className="typed-note-hero-title" data-testid="typed-note-primary">{primaryText || title || activeNoteType.name}</h2>
                {secondaryText && <p className="typed-note-hero-subtitle">{secondaryText}</p>}
              </div>
            </>
          )}

          {headerTemplate?.kind === 'default' && (
            <div className="typed-note-hero-default">
              <h2 className="typed-note-hero-title">{title || primaryText || activeNoteType.name}</h2>
              {secondaryText && <p className="typed-note-hero-subtitle">{secondaryText}</p>}
            </div>
          )}

          <div className="typed-note-fields-grid">
            {noteTypeDefinition.fields.map(field => {
              const value = headerProps[field.id]

              if (field.kind === 'boolean') {
                return (
                  <label className="typed-note-field" key={field.id}>
                    <span>{field.label}</span>
                    <label className="typed-note-checkbox">
                      <input checked={value === true} onChange={(event) => onHeaderPropChange(field.id, event.target.checked)} type="checkbox" />
                      <span>{value === true ? 'Да' : 'Нет'}</span>
                    </label>
                  </label>
                )
              }

              if (field.kind === 'long_text') {
                return (
                  <label className="typed-note-field typed-note-field-wide" key={field.id}>
                    <span>{field.label}</span>
                    <textarea className="typed-note-input typed-note-textarea" value={String(value ?? '')} onChange={(event) => onHeaderPropChange(field.id, event.target.value)} placeholder={field.placeholder ?? ''} />
                  </label>
                )
              }

              if (field.kind === 'select') {
                return (
                  <label className="typed-note-field" key={field.id}>
                    <span>{field.label}</span>
                    <select className="typed-note-input" value={String(value ?? '')} onChange={(event) => onHeaderPropChange(field.id, event.target.value)}>
                      <option value="">Не выбрано</option>
                      {(field.options ?? []).map(option => (
                        <option key={option} value={option}>{option}</option>
                      ))}
                    </select>
                  </label>
                )
              }

              return (
                <label className="typed-note-field" key={field.id}>
                  <span>{field.label}</span>
                  <input
                    className="typed-note-input"
                    data-testid={`typed-note-field-${field.id}`}
                    type={field.kind === 'number' ? 'number' : field.kind === 'date' ? 'date' : field.kind === 'image' ? 'url' : 'text'}
                    value={String(value ?? '')}
                    onChange={(event) => onHeaderPropChange(field.id, event.target.value)}
                    placeholder={field.placeholder ?? ''}
                  />
                </label>
              )
            })}
          </div>
          {validationError && <div className="typed-note-error">{validationError}</div>}
        </div>
      )}
    </div>
  )
}
