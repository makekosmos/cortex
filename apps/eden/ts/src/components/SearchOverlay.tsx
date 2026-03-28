import './SearchOverlay.css'
import { useCallback, useEffect, useRef, useState } from 'react'

interface SearchOverlayProps {
  isOpen: boolean
  query: string
  results: SearchResult[]
  entries: Entry[]
  onQueryChange: (value: string) => void
  onClose: () => void
  onResultSelect: (entryId: string) => Promise<void>
}

export default function SearchOverlay({
  isOpen,
  query,
  results,
  entries,
  onQueryChange,
  onClose,
  onResultSelect,
}: SearchOverlayProps) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [selectedIndex, setSelectedIndex] = useState(0)

  useEffect(() => {
    if (isOpen) {
      setSelectedIndex(0)
      setTimeout(() => {
        inputRef.current?.focus()
      }, 0)
    }
  }, [isOpen])

  useEffect(() => {
    setSelectedIndex(0)
  }, [results])

  const handleKeyDown = useCallback((event: React.KeyboardEvent) => {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        setSelectedIndex(prev => Math.min(prev + 1, results.length - 1))
        break
      case 'ArrowUp':
        event.preventDefault()
        setSelectedIndex(prev => Math.max(prev - 1, 0))
        break
      case 'Enter':
        event.preventDefault()
        if (results[selectedIndex]) {
          void onResultSelect(results[selectedIndex].entryId)
        }
        break
      case 'Escape':
        event.preventDefault()
        onClose()
        break
    }
  }, [results, selectedIndex, onResultSelect, onClose])

  const handleBackdropClick = useCallback((event: React.MouseEvent) => {
    if (event.target === event.currentTarget) {
      onClose()
    }
  }, [onClose])

  if (!isOpen) return null

  return (
    <div 
      className="search-overlay-backdrop" 
      onClick={handleBackdropClick}
      role="dialog"
      aria-modal="true"
    >
      <div className="search-overlay" onKeyDown={handleKeyDown}>
        <div className="search-overlay-input-wrap">
          <svg className="search-overlay-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
          <input
            ref={inputRef}
            type="text"
            className="search-overlay-input"
            placeholder="Поиск..."
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            autoFocus
          />
          {query && (
            <button className="search-overlay-clear-btn" onClick={onClose}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          )}
        </div>
        <div className="search-overlay-results">
          {query.trim() ? (
            results.length > 0 ? (
              results.map((result, index) => {
                const entry = entries.find(e => e.id === result.entryId)
                return (
                  <div
                    key={`${result.entryId}-${index}`}
                    className={`search-overlay-result-item ${selectedIndex === index ? 'is-selected' : ''}`}
                    onClick={() => void onResultSelect(result.entryId)}
                    onMouseEnter={() => setSelectedIndex(index)}
                  >
                    <div className="entry-title">{entry?.title || 'Без названия'}</div>
                    <div className="search-result-text">{result.text}</div>
                  </div>
                )
              })
            ) : (
              <div className="search-overlay-empty">Ничего не найдено</div>
            )
          ) : (
            <div className="search-overlay-empty">Начните вводить для поиска</div>
          )}
        </div>
        <div className="search-overlay-footer">
          <div className="search-overlay-hint">
            <kbd>↑</kbd><kbd>↓</kbd>
            <span>навигация</span>
          </div>
          <div className="search-overlay-hint">
            <kbd>Enter</kbd>
            <span>открыть</span>
          </div>
          <div className="search-overlay-hint">
            <kbd>Esc</kbd>
            <span>закрыть</span>
          </div>
        </div>
      </div>
    </div>
  )
}
