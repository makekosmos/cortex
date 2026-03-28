import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { createDefaultHeaderTemplate, createDefaultNoteTypeDefinition, parseHeaderTemplate, parseNoteTypeDefinition, type HeaderLayoutKind, type NoteTypeField } from '@/lib/typedNotes'
import { isSystemType } from '@/lib/systemTypes'

interface ObjectTypesSettingsProps {
  noteTypes: NoteType[];
  onNoteTypeSave: (noteType: Omit<NoteType, 'id' | 'created_at' | 'updated_at' | 'slug'> & { id?: string; slug?: string }) => Promise<SaveNoteTypeResult | { ok: false }>;
  onNoteTypeDelete: (noteTypeId: string) => Promise<void>;
}

type NoteTypeDraft = {
  id?: string;
  name: string;
  namePlural: string;
  icon: string;
  color: string;
  schema_json: string;
  header_template_json: string;
}

const ICON_NAMES = [
  'accessibility','add-circle','airplane','alarm','albums','alert-circle','american-football','analytics',
  'aperture','apps','archive','attach','backspace','bag','balloon','ban','bandage','bar-chart','barbell',
  'barcode','baseball','basket','basketball','beaker','bed','beer','bicycle','binoculars','bluetooth',
  'boat','body','bonfire','book','bookmark','bookmarks','bowling-ball','briefcase','browsers','brush',
  'bug','build','bulb','bus','business','cafe','calculator','calendar','calendar-clear','calendar-number',
  'call','camera','camera-reverse','car','car-sport','card','cart','cash','cellular','chatbox',
  'chatbox-ellipses','chatbubble','chatbubble-ellipses','chatbubbles','checkbox','checkmark-circle',
  'checkmark-done-circle','clipboard','close-circle','cloud','cloud-circle','cloud-done','cloud-download',
  'cloud-offline','cloud-upload','cloudy','cloudy-night','code','code-slash','cog','color-fill',
  'color-filter','color-palette','color-wand','compass','construct','contract','contrast','copy','create',
  'crop','cube','cut','desktop','diamond','dice','disc','document','document-attach','document-lock',
  'document-text','documents','download','duplicate','ear','earth','easel','egg','ellipse','enter','exit',
  'expand','extension-puzzle','eye','eye-off','eyedrop','fast-food','female','film','filter-circle',
  'finger-print','fish','fitness','flag','flame','flash','flash-off','flashlight','flask','flower',
  'folder','folder-open','football','footsteps','funnel','game-controller','gift','git-branch',
  'git-commit','git-compare','git-merge','git-network','git-pull-request','glasses','globe','golf',
  'grid','hammer','hand-left','hand-right','happy','hardware-chip','headset','heart','heart-circle',
  'heart-dislike','heart-half','help-buoy','help-circle','home','hourglass','ice-cream','id-card',
  'image','images','infinite','information-circle','journal','key','keypad','language','laptop','layers',
  'leaf','library','link','list','list-circle','locate','location','lock-closed','lock-open','log-in',
  'log-out','magnet','mail','mail-open','mail-unread','male','man','map','medal','medical','medkit',
  'megaphone','mic','mic-circle','moon','move','musical-note','musical-notes','navigate',
  'navigate-circle','newspaper','notifications','notifications-circle','nuclear','nutrition','options',
  'paper-plane','partly-sunny','pause','pause-circle','paw','pencil','people','people-circle','person',
  'person-add','person-circle','person-remove','phone-landscape','phone-portrait','pie-chart','pin',
  'pint','pizza','planet','play','play-circle','podium','power','pricetag','pricetags','print','prism',
  'pulse','push','qr-code','radio','rainy','reader','receipt','recording','refresh','reload',
  'remove-circle','repeat','resize','restaurant','ribbon','rocket','rose','sad','save','scale','scan',
  'school','search','send','server','settings','shapes','share','share-social','shield',
  'shield-checkmark','shield-half','shirt','shuffle','skull','snow','sparkles','speedometer','square',
  'star','star-half','stats-chart','stop','stopwatch','storefront','subway','sunny','swap-horizontal',
  'swap-vertical','sync','tablet-landscape','tablet-portrait','telescope','tennisball','terminal','text',
  'thermometer','thumbs-down','thumbs-up','thunderstorm','ticket','time','timer','today','toggle',
  'trail-sign','train','trash','trash-bin','trending-down','trending-up','triangle','trophy','tv',
  'umbrella','unlink','videocam','volume-high','volume-low','volume-medium','volume-mute','walk',
  'wallet','warning','watch','water','wifi','wine','woman',
]

const newDraft = (): NoteTypeDraft => ({
  name: '',
  namePlural: '',
  icon: 'document',
  color: '#2aa7ee',
  schema_json: JSON.stringify(createDefaultNoteTypeDefinition()),
  header_template_json: JSON.stringify(createDefaultHeaderTemplate('default')),
})

function parseDraftFields(draft: NoteTypeDraft) {
  return parseNoteTypeDefinition(draft.schema_json).fields
}

const builtInTypes = [
  { name: 'Страница', icon: 'document', color: '#2aa7ee' },
  { name: 'Тренировка', icon: 'barbell', color: '#f97316' },
  { name: 'Упражнение', icon: 'fitness', color: '#22c55e' },
]

function IconPicker({ currentIcon, color, onSelect, onClose }: {
  currentIcon: string;
  color: string;
  onSelect: (iconName: string) => void;
  onClose: () => void;
}) {
  const [filter, setFilter] = useState('')
  const ref = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => { inputRef.current?.focus() }, [])

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    const handleEsc = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('mousedown', handleClick)
    window.addEventListener('keydown', handleEsc)
    return () => { window.removeEventListener('mousedown', handleClick); window.removeEventListener('keydown', handleEsc) }
  }, [onClose])

  const filtered = filter
    ? ICON_NAMES.filter(n => n.includes(filter.toLowerCase()))
    : ICON_NAMES

  return (
    <div ref={ref} className="icon-picker-menu">
      <div className="icon-picker-head">
        <div className="icon-picker-tab active">Иконки</div>
      </div>
      <div className="icon-picker-filter">
        <input
          ref={inputRef}
          className="icon-picker-search"
          type="text"
          placeholder="Отфильтровать"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <div className="icon-picker-grid">
        {filtered.map((name) => (
          <button
            key={name}
            className={`icon-picker-item ${name === currentIcon ? 'active' : ''}`}
            onClick={() => { onSelect(name); onClose() }}
            title={name}
            type="button"
          >
            <img
              src={`/anytype/icon/type/default/${name}.svg`}
              alt={name}
              width={24}
              height={24}
              style={{ filter: `drop-shadow(0 0 0 ${color})` }}
              draggable={false}
            />
          </button>
        ))}
      </div>
    </div>
  )
}

export default function ObjectTypesSettings({ noteTypes, onNoteTypeSave, onNoteTypeDelete }: ObjectTypesSettingsProps) {
  const [selectedTypeId, setSelectedTypeId] = useState<string | null>(null)
  const [typeDraft, setTypeDraft] = useState<NoteTypeDraft | null>(null)
  const [typeError, setTypeError] = useState<string | null>(null)
  const [iconPickerOpen, setIconPickerOpen] = useState(false)
  const [filterQuery, setFilterQuery] = useState('')

  const activeFields = useMemo(() => {
    if (!typeDraft) return []
    try { return parseDraftFields(typeDraft) } catch { return [] }
  }, [typeDraft])

  const filteredTypes = useMemo(() => {
    const userTypes = noteTypes.filter(nt => !isSystemType(nt.id))
    const q = filterQuery.trim().toLowerCase()
    if (!q) return userTypes
    return userTypes.filter(nt => nt.name.toLowerCase().includes(q))
  }, [noteTypes, filterQuery])

  const updateDraftFields = (fields: NoteTypeField[]) => {
    setTypeDraft(current => current ? { ...current, schema_json: JSON.stringify({ fields }) } : current)
  }

  const updateHeaderTemplate = (patch: Partial<ReturnType<typeof parseHeaderTemplate>>) => {
    setTypeDraft(current => {
      if (!current) return current
      return { ...current, header_template_json: JSON.stringify({ ...parseHeaderTemplate(current.header_template_json), ...patch }) }
    })
  }

  const openTypeEditor = useCallback((noteType?: NoteType) => {
    if (!noteType) {
      setTypeDraft(newDraft())
      setSelectedTypeId(null)
      setTypeError(null)
      return
    }
    setSelectedTypeId(noteType.id)
    setTypeDraft({
      id: noteType.id,
      name: noteType.name,
      namePlural: noteType.slug || noteType.name,
      icon: noteType.icon ?? 'document',
      color: noteType.color ?? '#2aa7ee',
      schema_json: noteType.schema_json,
      header_template_json: noteType.header_template_json,
    })
    setTypeError(null)
  }, [])

  const closeTypeEditor = () => { setTypeDraft(null); setSelectedTypeId(null); setTypeError(null) }

  const submitTypeEditor = async () => {
    if (!typeDraft) return
    const fields = parseDraftFields(typeDraft)
    const currentTemplate = parseHeaderTemplate(typeDraft.header_template_json)
    const textFieldIds = fields.filter(f => f.kind === 'text' || f.kind === 'long_text').map(f => f.id)
    const imageFieldId = fields.find(f => f.kind === 'image')?.id ?? null
    const nextTemplate = {
      ...currentTemplate,
      imageFieldId: currentTemplate.imageFieldId ?? imageFieldId,
      primaryFieldIds: currentTemplate.primaryFieldIds?.length ? currentTemplate.primaryFieldIds : textFieldIds.slice(0, 2),
      secondaryFieldIds: currentTemplate.secondaryFieldIds?.length ? currentTemplate.secondaryFieldIds : textFieldIds.slice(2, 4),
    }
    const result = await onNoteTypeSave({
      id: typeDraft.id,
      name: typeDraft.name.trim(),
      slug: typeDraft.namePlural.trim() || typeDraft.name.trim(),
      icon: typeDraft.icon.trim() || null,
      color: typeDraft.color.trim() || null,
      schema_json: typeDraft.schema_json,
      header_template_json: JSON.stringify(nextTemplate),
    })
    if (!result.ok) {
      setTypeError('message' in result ? result.message : 'Не удалось сохранить')
      return
    }
    closeTypeEditor()
  }

  return (
    <div className="object-types-layout">
      {/* Left: type list */}
      <div className="object-types-list">
        <div className="object-types-list-head">
          <div className="object-types-filter">
            <input
              className="object-types-search"
              type="text"
              placeholder="Поиск"
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
            />
          </div>
          <button className="settings-btn-secondary object-types-new-btn" onClick={() => openTypeEditor()} type="button">
            Новый
          </button>
        </div>

        <div className="object-types-items">
          <div className="object-types-section-name">Системные типы</div>
          {builtInTypes.map((bt) => (
            <div className="object-types-item builtin" key={bt.name}>
              <img
                className="object-types-item-icon"
                src={`/anytype/icon/type/default/${bt.icon}.svg`}
                alt=""
                width={18}
                height={18}
                draggable={false}
              />
              <span className="object-types-item-name">{bt.name}</span>
            </div>
          ))}

          {filteredTypes.length > 0 && (
            <div className="object-types-section-name">Мои типы</div>
          )}
          {filteredTypes.map((noteType) => (
            <button
              key={noteType.id}
              className={`object-types-item ${selectedTypeId === noteType.id ? 'active' : ''}`}
              onClick={() => openTypeEditor(noteType)}
              type="button"
            >
              <img
                className="object-types-item-icon"
                src={`/anytype/icon/type/default/${noteType.icon || 'document'}.svg`}
                alt=""
                width={18}
                height={18}
                draggable={false}
              />
              <span className="object-types-item-name">{noteType.name}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Right: type editor */}
      <div className="object-types-editor">
        {typeDraft ? (
          <div className="type-editor">
            <div className="type-editor-head">
              <button className="settings-btn-secondary" onClick={closeTypeEditor} type="button">Отмена</button>
              <button className="settings-btn-primary" onClick={() => void submitTypeEditor()} type="button">Сохранить</button>
            </div>

            <div className="type-editor-body">
              {/* Title section */}
              <div className="type-editor-section">
                <div className="type-editor-label">Название типа</div>
                <div className="type-editor-name-row">
                  <div className="type-editor-icon-btn-wrap" style={{ position: 'relative' }}>
                    <button
                      className="type-editor-icon-btn"
                      onClick={() => setIconPickerOpen(!iconPickerOpen)}
                      type="button"
                      title="Выбрать иконку"
                    >
                      <img
                        src={`/anytype/icon/type/default/${typeDraft.icon || 'document'}.svg`}
                        alt=""
                        width={20}
                        height={20}
                        draggable={false}
                      />
                    </button>
                    {iconPickerOpen && (
                      <IconPicker
                        currentIcon={typeDraft.icon}
                        color={typeDraft.color}
                        onSelect={(name) => setTypeDraft({ ...typeDraft, icon: name })}
                        onClose={() => setIconPickerOpen(false)}
                      />
                    )}
                  </div>
                  <input
                    className="type-editor-name-input"
                    value={typeDraft.name}
                    onChange={(e) => setTypeDraft({ ...typeDraft, name: e.target.value })}
                    placeholder="например, Проект"
                  />
                </div>
              </div>

              <div className="type-editor-section">
                <div className="type-editor-label">Тип во множественном числе</div>
                <div className="type-editor-name-row">
                  <input
                    className="type-editor-name-input"
                    value={typeDraft.namePlural}
                    onChange={(e) => setTypeDraft({ ...typeDraft, namePlural: e.target.value })}
                    placeholder="например, Проекты"
                  />
                </div>
              </div>

              {/* Color */}
              <div className="type-editor-section">
                <div className="type-editor-label">Цвет иконки</div>
                <div className="type-editor-color-row">
                  <input type="color" value={typeDraft.color} onChange={(e) => setTypeDraft({ ...typeDraft, color: e.target.value })} className="settings-color-picker" />
                  <input className="type-editor-color-input" value={typeDraft.color} onChange={(e) => setTypeDraft({ ...typeDraft, color: e.target.value })} />
                </div>
              </div>

              {/* Layout */}
              <div className="type-editor-section">
                <div className="type-editor-label">Макет</div>
                <div className="type-editor-layout-items">
                  <div className="type-editor-layout-row">
                    <span>Тип макета</span>
                    <select className="settings-select-inline" value={parseHeaderTemplate(typeDraft.header_template_json).kind} onChange={(e) => updateHeaderTemplate({ kind: e.target.value as HeaderLayoutKind })}>
                      <option value="default">Обычный</option>
                      <option value="centered_profile">Портрет по центру</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* Fields / Relations */}
              <div className="type-editor-section">
                <div className="type-editor-section-title-row">
                  <div className="type-editor-label">Свойства</div>
                  <button className="settings-btn-secondary" onClick={() => updateDraftFields([...activeFields, { id: `field_${activeFields.length + 1}`, label: 'Новое поле', kind: 'text', required: false }])} type="button">+</button>
                </div>
                <div className="type-editor-fields">
                  {activeFields.map((field, index) => (
                    <div className="type-editor-field-row" key={field.id}>
                      <input className="type-editor-field-input" value={field.label} onChange={(e) => {
                        const next = [...activeFields]
                        next[index] = { ...field, label: e.target.value, id: e.target.value.trim().replace(/\s+/g, '_') || field.id }
                        updateDraftFields(next)
                      }} placeholder="Название поля" />
                      <select className="settings-select-inline" value={field.kind} onChange={(e) => {
                        const next = [...activeFields]
                        next[index] = { ...field, kind: e.target.value as NoteFieldKind }
                        updateDraftFields(next)
                      }}>
                        <option value="text">Строка</option>
                        <option value="long_text">Длинный текст</option>
                        <option value="number">Число</option>
                        <option value="date">Дата</option>
                        <option value="boolean">Да / Нет</option>
                        <option value="select">Выбор</option>
                        <option value="image">Изображение</option>
                      </select>
                      <button className="settings-btn-danger-sm" onClick={() => updateDraftFields(activeFields.filter(f => f.id !== field.id))} type="button">×</button>
                    </div>
                  ))}
                </div>
              </div>

              {/* Delete */}
              {typeDraft.id && (
                <div className="type-editor-section">
                  <button className="settings-btn-danger" onClick={() => { void onNoteTypeDelete(typeDraft.id!); closeTypeEditor() }} type="button">
                    Удалить тип
                  </button>
                </div>
              )}

              {typeError && <div className="dialog-error">{typeError}</div>}
            </div>
          </div>
        ) : (
          <div className="type-editor-empty">
            Выберите тип объекта или создайте новый
          </div>
        )}
      </div>
    </div>
  )
}
