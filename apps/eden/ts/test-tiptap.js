import { JSDOM } from 'jsdom'
const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>')
global.window = dom.window
global.document = dom.window.document
global.HTMLElement = dom.window.HTMLElement

import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { Markdown } from '@tiptap/markdown'

const editor = new Editor({
  extensions: [StarterKit, Markdown],
  content: ''
})

const json = editor.markdown.parse('```js\nconsole.log("hello")\n```')
console.log(JSON.stringify(json, null, 2))
