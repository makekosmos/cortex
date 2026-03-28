import { useEditor, EditorContent, Editor as TiptapEditor, ReactRenderer, Range, NodeViewWrapper, NodeViewContent, ReactNodeViewRenderer } from '@tiptap/react'
import StarterKit from '@tiptap/starter-kit'
import { Markdown } from '@tiptap/markdown'
import Placeholder from '@tiptap/extension-placeholder'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { all, createLowlight } from 'lowlight'
import { Wikilink } from './Wikilink'
import { WikilinkList, WikilinkListRef } from './WikilinkList'
import { SlashCommand } from './SlashCommand'
import { SlashCommandList, SlashCommandListRef } from './SlashCommandList'
import tippy, { Instance } from 'tippy.js'
import Typography from '@tiptap/extension-typography'
import { useEffect, useState, useCallback, useMemo, useRef, type CSSProperties } from 'react'
import TypedHeader from '@/components/typed-notes/TypedHeader'
import { createDefaultHeaderProps, parseHeaderTemplate, safeParseHeaderProps, validateHeaderProps } from '@/lib/typedNotes'
import './Editor.css'

const lowlight = createLowlight(all)

const codeBlockLanguages: { id: string; name: string }[] = [
  { id: 'plaintext', name: 'Plain Text' },
  { id: 'arduino', name: 'Arduino' },
  { id: 'bash', name: 'Bash' },
  { id: 'c', name: 'C' },
  { id: 'cpp', name: 'C++' },
  { id: 'csharp', name: 'C#' },
  { id: 'clojure', name: 'Clojure' },
  { id: 'coffeescript', name: 'CoffeeScript' },
  { id: 'css', name: 'CSS' },
  { id: 'dart', name: 'Dart' },
  { id: 'diff', name: 'Diff' },
  { id: 'dockerfile', name: 'Dockerfile' },
  { id: 'elixir', name: 'Elixir' },
  { id: 'elm', name: 'Elm' },
  { id: 'erlang', name: 'Erlang' },
  { id: 'fortran', name: 'Fortran' },
  { id: 'fsharp', name: 'F#' },
  { id: 'glsl', name: 'GLSL' },
  { id: 'go', name: 'Go' },
  { id: 'graphql', name: 'GraphQL' },
  { id: 'groovy', name: 'Groovy' },
  { id: 'haskell', name: 'Haskell' },
  { id: 'html', name: 'HTML' },
  { id: 'ini', name: 'INI' },
  { id: 'java', name: 'Java' },
  { id: 'javascript', name: 'JavaScript' },
  { id: 'json', name: 'JSON' },
  { id: 'julia', name: 'Julia' },
  { id: 'kotlin', name: 'Kotlin' },
  { id: 'latex', name: 'LaTeX' },
  { id: 'less', name: 'Less' },
  { id: 'lisp', name: 'Lisp' },
  { id: 'lua', name: 'Lua' },
  { id: 'makefile', name: 'Makefile' },
  { id: 'markdown', name: 'Markdown' },
  { id: 'matlab', name: 'MATLAB' },
  { id: 'nix', name: 'Nix' },
  { id: 'objectivec', name: 'Objective-C' },
  { id: 'ocaml', name: 'OCaml' },
  { id: 'pascal', name: 'Pascal' },
  { id: 'perl', name: 'Perl' },
  { id: 'php', name: 'PHP' },
  { id: 'powershell', name: 'PowerShell' },
  { id: 'protobuf', name: 'Protobuf' },
  { id: 'python', name: 'Python' },
  { id: 'r', name: 'R' },
  { id: 'ruby', name: 'Ruby' },
  { id: 'rust', name: 'Rust' },
  { id: 'scala', name: 'Scala' },
  { id: 'scss', name: 'SCSS' },
  { id: 'shell', name: 'Shell' },
  { id: 'sql', name: 'SQL' },
  { id: 'swift', name: 'Swift' },
  { id: 'toml', name: 'TOML' },
  { id: 'typescript', name: 'TypeScript' },
  { id: 'vbnet', name: 'VB.NET' },
  { id: 'verilog', name: 'Verilog' },
  { id: 'vhdl', name: 'VHDL' },
  { id: 'wasm', name: 'WebAssembly' },
  { id: 'xml', name: 'XML' },
  { id: 'yaml', name: 'YAML' },
  { id: 'zig', name: 'Zig' },
]

const codeBlockLanguageMap = new Map(codeBlockLanguages.map(lang => [lang.id, lang.name]))

const codeLanguageAliases: Record<string, string> = {
  js: 'javascript',
  ts: 'typescript',
  py: 'python',
  rb: 'ruby',
  rs: 'rust',
  cs: 'csharp',
  'c++': 'cpp',
  'c#': 'csharp',
  'f#': 'fsharp',
  sh: 'bash',
  zsh: 'bash',
  yml: 'yaml',
  tex: 'latex',
  kt: 'kotlin',
  objc: 'objectivec',
  'objective-c': 'objectivec',
  ps: 'powershell',
  ps1: 'powershell',
  proto: 'protobuf',
  hs: 'haskell',
  ex: 'elixir',
  erl: 'erlang',
  ml: 'ocaml',
  vb: 'vbnet',
  asm: 'wasm',
  plain: 'plaintext',
  text: 'plaintext',
  txt: 'plaintext',
}

function resolveLanguageId(language: string | null | undefined): string {
  if (!language || !language.trim()) return 'plaintext'
  const lower = language.toLowerCase()
  return codeLanguageAliases[lower] ?? lower
}

function getCodeLanguageDisplayName(languageId: string) {
  return codeBlockLanguageMap.get(languageId) ?? languageId
}

function CodeBlockView({ node, updateAttributes }: { node: any; updateAttributes: (attrs: Record<string, any>) => void }) {
  const [isMenuOpen, setIsMenuOpen] = useState(false)
  const [langSearch, setLangSearch] = useState('')
  const langSearchRef = useRef<HTMLInputElement>(null)

  const language = resolveLanguageId(node.attrs.language)
  const isWrapped = node.attrs.wrap !== false

  useEffect(() => {
    if (!isMenuOpen) return
    const close = () => { setIsMenuOpen(false); setLangSearch('') }
    const timer = setTimeout(() => document.addEventListener('click', close), 0)
    return () => { clearTimeout(timer); document.removeEventListener('click', close) }
  }, [isMenuOpen])

  useEffect(() => {
    if (isMenuOpen) setTimeout(() => langSearchRef.current?.focus(), 0)
  }, [isMenuOpen])

  return (
    <NodeViewWrapper className="code-block-node">
      <div className="code-block-actions" contentEditable={false}>
        <div className="code-block-actions-left">
          <button
            className="code-block-lang-btn"
            onClick={(e) => {
              e.stopPropagation()
              setIsMenuOpen(!isMenuOpen)
              setLangSearch('')
            }}
            type="button"
          >
            {getCodeLanguageDisplayName(language)}
            <svg className="code-block-lang-arrow" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>
          {isMenuOpen && (
            <div className="code-tools-menu" onClick={(e) => e.stopPropagation()}>
              <input
                ref={langSearchRef}
                className="code-tools-search"
                type="text"
                placeholder="Search..."
                value={langSearch}
                onChange={(e) => setLangSearch(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Escape') { setIsMenuOpen(false); setLangSearch('') } }}
              />
              <div className="code-tools-languages">
                {codeBlockLanguages
                  .filter((lang) => {
                    if (!langSearch) return true
                    const q = langSearch.toLowerCase()
                    return lang.name.toLowerCase().includes(q) || lang.id.toLowerCase().includes(q)
                  })
                  .map((lang) => (
                    <button
                      key={lang.id}
                      className={`code-language-option ${lang.id === language ? 'is-active' : ''}`}
                      onClick={() => {
                        updateAttributes({ language: lang.id })
                        setIsMenuOpen(false)
                        setLangSearch('')
                      }}
                      type="button"
                    >
                      {lang.name}
                    </button>
                  ))}
              </div>
            </div>
          )}
        </div>
        <div className="code-block-actions-right">
          <button
            className="code-block-toolbar-btn"
            onClick={(e) => { e.stopPropagation(); updateAttributes({ wrap: !isWrapped }) }}
            title={isWrapped ? 'Disable wrapping' : 'Enable wrapping'}
            type="button"
          >
            {isWrapped ? 'Unwrap' : 'Wrap'}
          </button>
          <button
            className="code-block-toolbar-btn"
            onClick={(e) => { e.stopPropagation(); void navigator.clipboard.writeText(node.textContent) }}
            title="Copy code"
            type="button"
          >
            Copy
          </button>
        </div>
      </div>
      <pre data-wrap={isWrapped ? 'true' : 'false'}>
        <NodeViewContent as={"code" as any} className={`language-${language}`} />
      </pre>
    </NodeViewWrapper>
  )
}

const EdenCodeBlock = CodeBlockLowlight.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      wrap: {
        default: true,
        parseHTML: (element: HTMLElement) => element.getAttribute('data-wrap') !== 'false',
        renderHTML: (attributes: { wrap?: boolean }) => ({
          'data-wrap': attributes.wrap === false ? 'false' : 'true',
        }),
      },
    }
  },
  addNodeView() {
    return ReactNodeViewRenderer(CodeBlockView)
  },
})

interface EditorProps {
  entry: Entry;
  allEntries: Entry[];
  noteTypes: NoteType[];
  codeToolsSettings: CodeToolsSettings | null;
  onSave: (entry: Entry) => Promise<SaveEntryResult | null>;
  onNavigate: (entryId: string) => void;
}

export default function Editor({ entry, allEntries, noteTypes, codeToolsSettings, onSave, onNavigate }: EditorProps) {
  const [title, setTitle] = useState(entry.title)
  const [noteTypeId, setNoteTypeId] = useState(entry.type_id)
  const [headerLayout, setHeaderLayout] = useState(entry.header_layout)
  const [headerProps, setHeaderProps] = useState<Record<string, unknown>>(() => safeParseHeaderProps(null, entry.header_props_json))
  const [saveConflict, setSaveConflict] = useState<string | null>(null)
  const [headerValidationError, setHeaderValidationError] = useState<string | null>(null)
  const [isNoteTypeMenuOpen, setIsNoteTypeMenuOpen] = useState(false)
  const [, setLintResultsByPos] = useState<Record<number, CodeLintDiagnostic[]>>({})
  const allEntriesRef = useRef(allEntries)
  const editorContentAreaRef = useRef<HTMLDivElement | null>(null)
  const noteTypeMenuRef = useRef<HTMLDivElement | null>(null)
  const lintRunIdRef = useRef(0)
  const lastSavedSnapshotRef = useRef('')
  const saveRunIdRef = useRef(0)
  const activeNoteType = useMemo(() => noteTypes.find(noteType => noteType.id === noteTypeId) ?? null, [noteTypeId, noteTypes])

  useEffect(() => {
    allEntriesRef.current = allEntries
  }, [allEntries])

  useEffect(() => {
    if (!isNoteTypeMenuOpen) {
      return
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (noteTypeMenuRef.current?.contains(event.target as Node)) {
        return
      }

      setIsNoteTypeMenuOpen(false)
    }

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsNoteTypeMenuOpen(false)
      }
    }

    window.addEventListener('mousedown', handlePointerDown)
    window.addEventListener('keydown', handleEscape)

    return () => {
      window.removeEventListener('mousedown', handlePointerDown)
      window.removeEventListener('keydown', handleEscape)
    }
  }, [isNoteTypeMenuOpen])

  const extensions = useMemo(() => [
    StarterKit.configure({
      codeBlock: false,
    }),
    Markdown,
    Placeholder.configure({
      placeholder: 'Начни писать что-нибудь интересное...',
    }),
    EdenCodeBlock.configure({
      lowlight,
      enableTabIndentation: true,
      tabSize: 4,
      defaultLanguage: null,
    }),
    Typography,
    Wikilink.configure({
      suggestion: {
        items: ({ query }: { query: string }) => {
          return allEntriesRef.current
            .filter(item => item.title.toLowerCase().includes(query.toLowerCase()))
            .slice(0, 10)
        },
        render: () => {
          let component: ReactRenderer<WikilinkListRef, any>;
          let popup: Instance[];

          return {
            onStart: (props: any) => {
              component = new ReactRenderer(WikilinkList, {
                props,
                editor: props.editor,
              })

              if (!props.clientRect) {
                return
              }

              popup = tippy('body', {
                getReferenceClientRect: props.clientRect,
                appendTo: () => document.body,
                content: component.element,
                showOnCreate: true,
                interactive: true,
                trigger: 'manual',
                placement: 'bottom-start',
              })
            },

            onUpdate(props: any) {
              component.updateProps(props)

              if (!props.clientRect) {
                return
              }

              popup[0].setProps({
                getReferenceClientRect: props.clientRect,
              })
            },

            onKeyDown(props: any) {
              if (props.event.key === 'Escape') {
                popup[0].hide()
                return true
              }
              return component.ref?.onKeyDown(props) || false
            },

            onExit() {
              popup[0].destroy()
              component.destroy()
            },
          }
        },
      },
    }),
    SlashCommand.configure({
      suggestion: {
        items: ({ query }: { query: string }) => {
          return [
            {
              title: 'Заголовок 1',
              description: 'Большой заголовок раздела',
              icon: 'H1',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).setNode('heading', { level: 1 }).run()
              },
            },
            {
              title: 'Заголовок 2',
              description: 'Средний заголовок',
              icon: 'H2',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).setNode('heading', { level: 2 }).run()
              },
            },
            {
              title: 'Текст',
              description: 'Обычный абзац',
              icon: 'P',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).setNode('paragraph').run()
              },
            },
            {
              title: 'Список',
              description: 'Маркированный список',
              icon: '•',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).toggleBulletList().run()
              },
            },
            {
              title: 'Код',
              description: 'Блок кода с подсветкой',
              icon: '{}',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).toggleCodeBlock().run()
              },
            },
            {
              title: 'Цитата',
              description: 'Блок цитирования',
              icon: '"',
              command: ({ editor, range }: { editor: TiptapEditor; range: Range }) => {
                editor.chain().focus().deleteRange(range).toggleBlockquote().run()
              },
            },
          ].filter(item => item.title.toLowerCase().includes(query.toLowerCase()))
        },
        render: () => {
          let component: ReactRenderer<SlashCommandListRef, any>;
          let popup: Instance[];

          return {
            onStart: (props: any) => {
              component = new ReactRenderer(SlashCommandList, {
                props,
                editor: props.editor,
              })

              if (!props.clientRect) {
                return
              }

              popup = tippy('body', {
                getReferenceClientRect: props.clientRect,
                appendTo: () => document.body,
                content: component.element,
                showOnCreate: true,
                interactive: true,
                trigger: 'manual',
                placement: 'bottom-start',
              })
            },

            onUpdate(props: any) {
              component.updateProps(props)

              if (!props.clientRect) {
                return
              }

              popup[0].setProps({
                getReferenceClientRect: props.clientRect,
              })
            },

            onKeyDown(props: any) {
              if (props.event.key === 'Escape') {
                popup[0].hide()
                return true
              }
              return component.ref?.onKeyDown(props) || false
            },

            onExit() {
              popup[0].destroy()
              component.destroy()
            },
          }
        },
      },
    }),
  ], [])

  const editor = useEditor({
    extensions,
    content: (() => {
      try {
        return entry.content_json ? JSON.parse(entry.content_json) : { type: 'doc', content: [{ type: 'paragraph' }] };
      } catch {
        return { type: 'doc', content: [{ type: 'paragraph' }] };
      }
    })(),
    autofocus: 'end',
    editable: true,
  })

  const applyCodeBlockText = useCallback((nodePos: number, nextCode: string) => {
    if (!editor) {
      return false
    }

    const codeBlockNode = editor.state.doc.nodeAt(nodePos)
    if (!codeBlockNode) {
      return false
    }

    const contentFrom = nodePos + 1
    const contentTo = nodePos + codeBlockNode.nodeSize - 1
    const transaction = editor.state.tr

    if (contentTo > contentFrom) {
      transaction.delete(contentFrom, contentTo)
    }

    if (nextCode.length > 0) {
      transaction.insertText(nextCode, contentFrom)
    }

    editor.view.dispatch(transaction)
    return true
  }, [editor])

  const formatAllCodeBlocks = useCallback(async () => {
    if (!editor || !codeToolsSettings) {
      return
    }

    const codeBlocks: Array<{ nodePos: number; code: string; language: string }> = []

    editor.state.doc.descendants((node, position) => {
      if (node.type.name === 'codeBlock') {
        codeBlocks.push({
          nodePos: position,
          code: node.textContent,
          language: resolveLanguageId((node.attrs as { language?: string | null }).language),
        })
      }

      return true
    })

    for (const block of [...codeBlocks].reverse()) {
      const result = await window.api.formatCodeBlock(block.language, block.code)
      if (result.code !== block.code) {
        applyCodeBlockText(block.nodePos, result.code)
      }
    }
  }, [applyCodeBlockText, codeToolsSettings, editor])

  const lintAllCodeBlocks = useCallback(async () => {
    if (!editor || !codeToolsSettings) {
      return
    }

    const runId = ++lintRunIdRef.current
    const codeBlocks: Array<{ nodePos: number; code: string; language: string }> = []

    editor.state.doc.descendants((node, position) => {
      if (node.type.name === 'codeBlock') {
        codeBlocks.push({
          nodePos: position,
          code: node.textContent,
          language: resolveLanguageId((node.attrs as { language?: string | null }).language),
        })
      }

      return true
    })

    const nextResults: Record<number, CodeLintDiagnostic[]> = {}

    for (const block of codeBlocks) {
      const result = await window.api.lintCodeBlock(block.language, block.code)
      if (!result.ok || result.diagnostics.length > 0) {
        nextResults[block.nodePos] = result.diagnostics
      }
    }

    if (runId !== lintRunIdRef.current) {
      return
    }

    setLintResultsByPos(nextResults)
  }, [codeToolsSettings, editor])

  useEffect(() => {
    if (!editor) return;

    try {
      const parsed = entry.content_json ? JSON.parse(entry.content_json) : { type: 'doc', content: [{ type: 'paragraph' }] };
      const currentContent = editor.getJSON();
      if (JSON.stringify(currentContent) !== JSON.stringify(parsed)) {
        editor.commands.setContent(parsed);
      }
    } catch (e) {
      console.error('Failed to parse content_json', e);
    }
  }, [editor, entry.content_json]);

  useEffect(() => {
    setLintResultsByPos({})
  }, [entry.id])

  useEffect(() => {
    setNoteTypeId(entry.type_id)
    setHeaderLayout(entry.header_layout)
    const nextNoteType = noteTypes.find(noteType => noteType.id === entry.type_id) ?? null
    setHeaderProps(safeParseHeaderProps(nextNoteType, entry.header_props_json))
    setHeaderValidationError(null)
  }, [entry.header_layout, entry.header_props_json, entry.id, entry.type_id, noteTypes])

  const getCurrentSnapshot = useCallback(() => {
    if (!editor) {
      return null
    }

    return JSON.stringify({
      title: title || 'Без названия',
      noteTypeId,
      headerLayout,
      headerProps,
      doc: editor.getJSON(),
      markdown: editor.getMarkdown(),
    })
  }, [editor, headerLayout, headerProps, noteTypeId, title])

  const handleNoteTypeChange = useCallback((nextTypeId: string) => {
    const nextNoteType = noteTypes.find(noteType => noteType.id === nextTypeId) ?? null
    setNoteTypeId(nextTypeId || null)
    setHeaderLayout(nextNoteType ? parseHeaderTemplate(nextNoteType.header_template_json).kind : null)
    setHeaderProps(createDefaultHeaderProps(nextNoteType))
    setHeaderValidationError(null)
    setIsNoteTypeMenuOpen(false)
  }, [noteTypes])

  const handleHeaderPropChange = useCallback((fieldId: string, value: unknown) => {
    setHeaderProps(currentProps => ({
      ...currentProps,
      [fieldId]: value,
    }))
  }, [])

  const save = useCallback(async () => {
    if (!editor) return

    const headerValidation = validateHeaderProps(activeNoteType, headerProps)
    if (!headerValidation.success) {
      setHeaderValidationError('Проверьте поля верхушки заметки')
      return
    }

    setHeaderValidationError(null)

    const snapshotBeforeSave = getCurrentSnapshot()
    if (!snapshotBeforeSave || lastSavedSnapshotRef.current === snapshotBeforeSave) {
      return
    }

    const saveRunId = ++saveRunIdRef.current

    if (codeToolsSettings?.formatOnSave) {
      await formatAllCodeBlocks()
    }

    if (codeToolsSettings?.lintTrigger === 'on_save') {
      await lintAllCodeBlocks()
    }

    const content_json = JSON.stringify(editor.getJSON())

    const saveResult = await onSave({
      ...entry,
      title: title || 'Без названия',
      content_json,
      type_id: noteTypeId,
      header_layout: headerLayout,
      header_props_json: JSON.stringify(headerValidation.data),
      schema_version: 1,
      updated_at: Date.now(),
    })

    if (!saveResult) {
      return
    }

    if (saveRunId !== saveRunIdRef.current) {
      return
    }

    if (!saveResult.ok && saveResult.reason === 'duplicate_title') {
      setSaveConflict('Заметка с таким названием уже есть в этой папке')
      return
    }

    if (!saveResult.ok && saveResult.reason === 'invalid_type_metadata') {
      setHeaderValidationError(saveResult.message)
      return
    }

    setSaveConflict(null)
    lastSavedSnapshotRef.current = getCurrentSnapshot() ?? snapshotBeforeSave
  }, [activeNoteType, codeToolsSettings?.formatOnSave, codeToolsSettings?.lintTrigger, editor, entry, formatAllCodeBlocks, getCurrentSnapshot, headerLayout, headerProps, lintAllCodeBlocks, noteTypeId, onSave, title])

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally reacts only to title changes
  useEffect(() => {
    setSaveConflict(null)
  }, [title])

  useEffect(() => {
    setSaveConflict(null)
  }, [entry.id])

  useEffect(() => {
    if (!editor) return

    lastSavedSnapshotRef.current = getCurrentSnapshot() ?? ''
  }, [editor, entry.id, entry.updated_at, getCurrentSnapshot])

  useEffect(() => {
    if (!editor) return

    const handleEditorClick = (event: MouseEvent) => {
      const target = event.target as HTMLElement
      const wikilink = target.closest('span[data-id]')
      if (wikilink) {
        const entryId = wikilink.getAttribute('data-id')
        if (entryId) {
          onNavigate(entryId)
        }
      }
    }

    const editorView = editor.view.dom
    editorView.addEventListener('click', handleEditorClick)
    return () => editorView.removeEventListener('click', handleEditorClick)
  }, [editor, onNavigate])

  useEffect(() => {
    if (!editor) return

    const timer = setTimeout(() => {
      void save();
    }, 800);

    return () => clearTimeout(timer);
  }, [editor?.state.doc, title, save])

  useEffect(() => {
    if (!editor || codeToolsSettings?.lintTrigger !== 'on_idle') return

    const timer = window.setTimeout(() => {
      void lintAllCodeBlocks()
    }, 1000)

    return () => window.clearTimeout(timer)
  }, [codeToolsSettings?.lintTrigger, editor?.state.doc, lintAllCodeBlocks])

  useEffect(() => {
    if (!editor) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        void save()
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [editor, save])

  return (
    <div
      className="editor-wrapper"
    >
      <div className="editor-header">
        <div className="editor-rail editor-header-rail">
          <div className="editor-header-main">
            <TypedHeader
              activeNoteType={activeNoteType}
              title={title}
              headerProps={headerProps}
              validationError={headerValidationError}
              onHeaderPropChange={handleHeaderPropChange}
            />
            <input
              className="title-input"
              value={title}
              onChange={e => setTitle(e.target.value)}
              placeholder="Заголовок"
            />
            <div className="note-type-inline" ref={noteTypeMenuRef}>
              <button
                className="note-type-trigger"
                data-testid="typed-note-trigger"
                onClick={() => setIsNoteTypeMenuOpen((current) => !current)}
                style={{ '--note-type-accent': activeNoteType?.color ?? 'var(--text-tertiary)' } as CSSProperties}
                type="button"
              >
                {activeNoteType?.name ?? 'Обычная заметка'}
              </button>
              {isNoteTypeMenuOpen && (
                <div className="note-type-menu" data-testid="typed-note-menu">
                  <button className={`note-type-menu-item ${noteTypeId ? '' : 'is-active'}`} onClick={() => handleNoteTypeChange('')} type="button">
                    Обычная заметка
                  </button>
                  {noteTypes.map((noteType) => (
                    <button
                      key={noteType.id}
                      className={`note-type-menu-item ${noteType.id === noteTypeId ? 'is-active' : ''}`}
                      onClick={() => handleNoteTypeChange(noteType.id)}
                      type="button"
                    >
                      {noteType.name}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="editor-header-actions">
            {saveConflict && <span className="save-conflict-badge">{saveConflict}</span>}
          </div>
        </div>
      </div>
      <div
        ref={editorContentAreaRef}
        className="editor-content-area"
        onClick={() => {
          editor?.commands.focus()
        }}
      >
        <div className="editor-rail editor-content-rail">
          <EditorContent editor={editor} />
        </div>
      </div>
    </div>
  )
}
