import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { Editor, Range } from '@tiptap/react'
import { PluginKey } from '@tiptap/pm/state'

const slashCommandSuggestionPluginKey = new PluginKey('slashCommandSuggestion')

export const SlashCommand = Extension.create({
  name: 'slashCommand',

  addOptions() {
    return {
      suggestion: {
        char: '/',
        command: ({ editor, range, props }: { editor: Editor; range: Range; props: { command: (p: { editor: Editor; range: Range }) => void } }) => {
          props.command({ editor, range })
        },
      },
    }
  },

  addProseMirrorPlugins() {
    return [
      Suggestion({
        editor: this.editor,
        pluginKey: slashCommandSuggestionPluginKey,
        ...this.options.suggestion,
      }),
    ]
  },
})
