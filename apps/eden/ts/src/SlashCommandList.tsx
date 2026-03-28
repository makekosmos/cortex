import { forwardRef, useEffect, useImperativeHandle, useState } from 'react'
import { Editor, Range } from '@tiptap/react'

interface SlashCommandItem {
  title: string;
  description: string;
  icon: string;
  command: (props: { editor: Editor; range: Range }) => void;
}

interface SlashCommandListProps {
  items: SlashCommandItem[];
  command: (item: SlashCommandItem) => void;
}

export interface SlashCommandListRef {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

export const SlashCommandList = forwardRef<SlashCommandListRef, SlashCommandListProps>((props, ref) => {
  const [selectedIndex, setSelectedIndex] = useState(0)

  const selectItem = (index: number) => {
    const item = props.items[index]
    if (item) {
      props.command(item)
    }
  }

  useEffect(() => setSelectedIndex(0), [props.items])

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (event.key === 'ArrowUp') {
        setSelectedIndex((selectedIndex + props.items.length - 1) % props.items.length)
        return true
      }
      if (event.key === 'ArrowDown') {
        setSelectedIndex((selectedIndex + 1) % props.items.length)
        return true
      }
      if (event.key === 'Enter') {
        selectItem(selectedIndex)
        return true
      }
      return false
    },
  }))

  return (
    <div className="slash-commands">
      {props.items.length
        ? props.items.map((item, index: number) => (
          <button
            className={`suggestion-item ${index === selectedIndex ? 'is-selected' : ''}`}
            key={index}
            onClick={() => selectItem(index)}
          >
            <span className="command-icon">{item.icon}</span>
            <div className="command-info">
              <span className="command-title">{item.title}</span>
              <span className="command-description">{item.description}</span>
            </div>
          </button>
        ))
        : <div className="suggestion-item no-result">Команд не найдено</div>
      }
    </div>
  )
})

SlashCommandList.displayName = 'SlashCommandList'
