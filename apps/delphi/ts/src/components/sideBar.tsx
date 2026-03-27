import { Archive, Calendar, Home, Settings } from 'lucide-react';
import SideBarButton from '@/components/SideBarButton';

export default function SideBar() {
  return (
    <aside
      style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
      className="left-0 flex min-h-0 w-fit flex-col justify-between border-r border-r-(--border) bg-(--sidebar) p-2"
    >
      <div>
        <SideBarButton icon={Home} to="/" />
        <SideBarButton icon={Calendar} to="/upcoming" />
        <SideBarButton icon={Archive} to="/completed" />
      </div>

      <div className="flex flex-col gap-2">
        <SideBarButton icon={Settings} to="/settings" />
      </div>
    </aside>
  );
}
