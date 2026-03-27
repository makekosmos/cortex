import type { LucideIcon } from 'lucide-react';
import { Link } from 'react-router-dom';

type SideBarButtonProps = {
  icon: LucideIcon;
  to?: string;
  onClick?: () => void;
};

export default function SideBarButton({
  icon: Icon,
  to,
  onClick,
}: SideBarButtonProps) {
  const className =
    'flex h-14 w-14 cursor-pointer items-center justify-center rounded-xl p-1 align-middle hover:bg-(--secondary) transition-colors';

  if (to) {
    return (
      <Link to={to} className={className}>
        <Icon size={24} />
      </Link>
    );
  }

  return (
    <button type="button" onClick={onClick} className={className}>
      <Icon size={24} />
    </button>
  );
}
