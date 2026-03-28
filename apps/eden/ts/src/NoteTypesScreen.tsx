import { useMemo, useState } from 'react'
import { createDefaultHeaderTemplate, createDefaultNoteTypeDefinition, parseHeaderTemplate, parseNoteTypeDefinition, type HeaderLayoutKind, type NoteTypeField } from '@/lib/typedNotes'

interface NoteTypesScreenProps {
  noteTypes: NoteType[];
  onBack: () => void;
  onNoteTypeSave: (noteType: Omit<NoteType, 'id' | 'created_at' | 'updated_at' | 'slug'> & { id?: string; slug?: string }) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
}

type NoteTypeDraft = {
  id?: string;
  name: string;
  icon: string;
  color: string;
  schema_json: string;
  header_template_json: string;
}

const newDraft = (): NoteTypeDraft => ({
  name: '',
  icon: '✦',
  color: '#7fb7ff',
  schema_json: JSON.stringify(createDefaultNoteTypeDefinition()),
  header_template_json: JSON.stringify(createDefaultHeaderTemplate('default')),
})

function parseDraftFields(draft: NoteTypeDraft) {
  return parseNoteTypeDefinition(draft.schema_json).fields
}

export default function NoteTypesScreen({ noteTypes, onBack, onNoteTypeSave, onNoteTypeDelete }: NoteTypesScreenProps) {
  const [typeDraft, setTypeDraft] = useState<NoteTypeDraft | null>(null)
  const [typeError, setTypeError] = useState<string | null>(null)

  const activeFields = useMemo(() => {
    if (!typeDraft) return []
    try {
      return parseDraftFields(typeDraft)
    } catch {
      return []
    }
  }, [typeDraft])

  const updateDraftFields = (fields: NoteTypeField[]) => {
    setTypeDraft(current => current ? {
      ...current,
      schema_json: JSON.stringify({ fields }),
    } : current)
  }

  const updateHeaderTemplate = (patch: Partial<ReturnType<typeof parseHeaderTemplate>>) => {
    setTypeDraft(current => {
      if (!current) return current
      const nextTemplate = {
        ...parseHeaderTemplate(current.header_template_json),
        ...patch,
      }
      return {
        ...current,
        header_template_json: JSON.stringify(nextTemplate),
      }
    })
  }

  const openTypeEditor = (noteType?: NoteType) => {
    if (!noteType) {
      setTypeDraft(newDraft())
      setTypeError(null)
      return
    }

    setTypeDraft({
      id: noteType.id,
      name: noteType.name,
      icon: noteType.icon ?? '✦',
      color: noteType.color ?? '#7fb7ff',
      schema_json: noteType.schema_json,
      header_template_json: noteType.header_template_json,
    })
    setTypeError(null)
  }

  const closeTypeEditor = () => {
    setTypeDraft(null)
    setTypeError(null)
  }

  const submitTypeEditor = async () => {
    if (!typeDraft) return

    const fields = parseDraftFields(typeDraft)
    const currentTemplate = parseHeaderTemplate(typeDraft.header_template_json)
    const textFieldIds = fields.filter(field => field.kind === 'text' || field.kind === 'long_text').map(field => field.id)
    const imageFieldId = fields.find(field => field.kind === 'image')?.id ?? null
    const nextTemplate = {
      ...currentTemplate,
      imageFieldId: currentTemplate.imageFieldId ?? imageFieldId,
      primaryFieldIds: currentTemplate.primaryFieldIds?.length ? currentTemplate.primaryFieldIds : textFieldIds.slice(0, 2),
      secondaryFieldIds: currentTemplate.secondaryFieldIds?.length ? currentTemplate.secondaryFieldIds : textFieldIds.slice(2, 4),
    }

    const result = await onNoteTypeSave({
      id: typeDraft.id,
      name: typeDraft.name.trim(),
      slug: typeDraft.name.trim(),
      icon: typeDraft.icon.trim() || null,
      color: typeDraft.color.trim() || null,
      schema_json: typeDraft.schema_json,
      header_template_json: JSON.stringify(nextTemplate),
    })

    if (!result.ok) {
      setTypeError('message' in result ? result.message : 'Не удалось сохранить тип заметки')
      return
    }

    closeTypeEditor()
  }

  return (
    <section className="settings-screen">
      <div className="settings-rail">
        <div className="settings-header">
          <div>
            <p className="settings-kicker">Типы</p>
            <h1 className="settings-title">Типы заметок</h1>
            <p className="settings-subtitle">Создавай свои типы с полями, иконкой и визуальной верхушкой.</p>
          </div>
          <div className="settings-header-actions">
            <button className="settings-primary-btn" data-testid="create-note-type" onClick={() => openTypeEditor()} type="button">Создать тип заметки</button>
            <button className="settings-back-btn" onClick={onBack} type="button">К настройкам</button>
          </div>
        </div>

        <div className="settings-grid settings-grid-full">
          <section className="settings-card settings-card-wide">
            <div className="settings-type-list">
              {noteTypes.map((noteType) => (
                <div className="settings-type-item" key={noteType.id}>
                  <div className="settings-type-meta">
                    <span className="settings-type-icon">{noteType.icon ?? '✦'}</span>
                    <div>
                      <strong>{noteType.name}</strong>
                      <p>{parseNoteTypeDefinition(noteType.schema_json).fields.length} полей</p>
                    </div>
                  </div>
                  <div className="settings-type-actions">
                    <button className="settings-secondary-btn" onClick={() => openTypeEditor(noteType)} type="button">Изменить</button>
                    <button className="settings-danger-btn" onClick={() => void onNoteTypeDelete(noteType.id)} type="button">Удалить</button>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>

      {typeDraft && (
        <div className="dialog-backdrop" data-testid="note-type-dialog" onClick={closeTypeEditor}>
          <div className="dialog-card settings-type-dialog" onClick={(event) => event.stopPropagation()}>
            <div className="dialog-header">
              <div>
                <p className="dialog-kicker">Тип заметки</p>
                <h2 className="dialog-title">{typeDraft.id ? 'Изменить тип' : 'Новый тип'}</h2>
              </div>
              <button className="dialog-close-btn" onClick={closeTypeEditor} type="button">×</button>
            </div>
            <div className="settings-field-grid">
              <label className="settings-field">
                <span>Название</span>
                <input className="settings-text-input" data-testid="note-type-name" value={typeDraft.name} onChange={(event) => setTypeDraft({ ...typeDraft, name: event.target.value })} />
              </label>
              <label className="settings-field">
                <span>Иконка</span>
                <input className="settings-text-input" value={typeDraft.icon} onChange={(event) => setTypeDraft({ ...typeDraft, icon: event.target.value })} />
              </label>
              <label className="settings-field">
                <span>Акцентный цвет</span>
                <input className="settings-text-input" value={typeDraft.color} onChange={(event) => setTypeDraft({ ...typeDraft, color: event.target.value })} />
              </label>
              <label className="settings-field">
                <span>Макет верхушки</span>
                <select className="settings-select" data-testid="note-type-layout" value={parseHeaderTemplate(typeDraft.header_template_json).kind} onChange={(event) => updateHeaderTemplate({ kind: event.target.value as HeaderLayoutKind })}>
                  <option value="default">Обычный</option>
                  <option value="centered_profile">Портрет по центру</option>
                </select>
              </label>
            </div>
            <div className="settings-type-builder">
              <div className="settings-card-header">
                <h2>Поля</h2>
                <p>Опиши свойства, которые будут в верхушке заметки этого типа.</p>
              </div>
              <div className="settings-type-fields">
                {activeFields.map((field, index) => (
                  <div className="settings-type-field-row" key={field.id}>
                    <input className="settings-text-input" data-testid={`note-type-field-label-${index}`} value={field.label} onChange={(event) => {
                      const nextFields = [...activeFields]
                      nextFields[index] = { ...field, label: event.target.value, id: event.target.value.trim().replace(/\s+/g, '_') || field.id }
                      updateDraftFields(nextFields)
                    }} placeholder="Название поля" />
                    <select className="settings-select" data-testid={`note-type-field-kind-${index}`} value={field.kind} onChange={(event) => {
                      const nextFields = [...activeFields]
                      nextFields[index] = { ...field, kind: event.target.value as NoteFieldKind }
                      updateDraftFields(nextFields)
                    }}>
                      <option value="text">Строка</option>
                      <option value="long_text">Длинный текст</option>
                      <option value="number">Число</option>
                      <option value="date">Дата</option>
                      <option value="boolean">Да / Нет</option>
                      <option value="select">Выбор</option>
                      <option value="image">Изображение</option>
                    </select>
                    <label className="settings-checkbox compact">
                      <input checked={field.required} onChange={(event) => {
                        const nextFields = [...activeFields]
                        nextFields[index] = { ...field, required: event.target.checked }
                        updateDraftFields(nextFields)
                      }} type="checkbox" />
                      <span>Обязательное</span>
                    </label>
                    <button className="settings-danger-btn" onClick={() => updateDraftFields(activeFields.filter(currentField => currentField.id !== field.id))} type="button">Убрать</button>
                  </div>
                ))}
              </div>
              <button className="settings-secondary-btn" data-testid="note-type-add-field" onClick={() => updateDraftFields([...activeFields, { id: `field_${activeFields.length + 1}`, label: 'Новое поле', kind: 'text', required: false }])} type="button">Добавить поле</button>
            </div>
            {typeError && <div className="dialog-error">{typeError}</div>}
            <div className="dialog-actions">
              <button className="dialog-secondary-btn" onClick={closeTypeEditor} type="button">Отмена</button>
              <button className="dialog-primary-btn" data-testid="note-type-submit" onClick={() => void submitTypeEditor()} type="button">Сохранить тип</button>
            </div>
          </div>
        </div>
      )}
    </section>
  )
}
