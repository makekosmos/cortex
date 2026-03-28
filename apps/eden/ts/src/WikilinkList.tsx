import { forwardRef, useEffect, useImperativeHandle, useState } from 'react'

interface WikilinkListProps {
  items: Array<{ id: string; title: string }>;
  command: (props: { id: string; label: string }) => void;
}

export interface WikilinkListRef {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

export const WikilinkList = forwardRef<WikilinkListRef, WikilinkListProps>((props, ref) => {
  const [selectedIndex, setSelectedIndex] = useState(0)

  const selectItem = (index: number) => {
    const item = props.items[index]
    if (item) {
      props.command({ id: item.id, label: item.title })
    }
  }

  const upHandler = () => {
    setSelectedIndex((selectedIndex + props.items.length - 1) % props.items.length)
  }

  const downHandler = () => {
    setSelectedIndex((selectedIndex + 1) % props.items.length)
  }

  const enterHandler = () => {
    selectItem(selectedIndex)
  }

  useEffect(() => setSelectedIndex(0), [props.items])

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (event.key === 'ArrowUp') {
        upHandler()
        return true
      }
      if (event.key === 'ArrowDown') {
        downHandler()
        return true
      }
      if (event.key === 'Enter') {
        enterHandler()
        return true
      }
      return false
    },
  }))

  return (
    <div className="wikilink-suggestions">
      {props.items.length
        ? props.items.map((item, index: number) => (
          <button
            className={`suggestion-item ${index === selectedIndex ? 'is-selected' : ''}`}
            key={item.id}
            onClick={() => selectItem(index)}
          >
            {item.title}
          </button>
        ))
        : <div className="suggestion-item no-result">Ничего не найдено</div>
      }
    </div>
  )
})

WikilinkList.displayName = 'WikilinkList'
