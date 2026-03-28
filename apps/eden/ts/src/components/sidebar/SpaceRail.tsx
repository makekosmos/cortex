export type SpaceId = 'my-space' | 'all-objects' | 'all-notes' | 'all-properties' | 'diary'

interface SpaceRailProps {
  activeSpace: SpaceId;
  onSelectSpace: (spaceId: SpaceId) => void;
}

const spaces: Array<{ id: SpaceId; label: string; icon: string }> = [
  { id: 'my-space', label: 'Мое пространство', icon: '🏡' },
  { id: 'all-objects', label: 'Все объекты', icon: '📚' },
  { id: 'all-notes', label: 'Все заметки', icon: '📝' },
]

export default function SpaceRail({
  activeSpace,
  onSelectSpace,
}: SpaceRailProps) {
  return (
    <aside className="spaces-rail" data-testid="spaces-rail">
      <div className="spaces-rail-list">
        {spaces.map((space) => (
          <button
            key={space.id}
            className={`space-rail-item ${activeSpace === space.id ? 'is-active' : ''}`}
            data-testid={`space-tab-${space.id}`}
            onClick={() => onSelectSpace(space.id)}
            title={space.label}
            type="button"
          >
            <span aria-hidden="true" className="space-rail-badge">{space.icon}</span>
          </button>
        ))}
      </div>
    </aside>
  )
}
