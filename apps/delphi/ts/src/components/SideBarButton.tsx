import type { LucideIcon } from "lucide-react";
import { Link, useLocation } from "react-router-dom";

type SideBarButtonProps = {
  icon: LucideIcon;
  to?: string;
  label?: string;
  onClick?: () => void;
};

export default function SideBarButton({
  icon: Icon,
  to,
  label,
  onClick,
}: SideBarButtonProps) {
  const location = useLocation();
  const isActive = to ? location.pathname === to : false;

  const base =
    "flex h-10 w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors";
  const activeClass = isActive
    ? "bg-(--accent) text-(--foreground) font-medium"
    : "text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)";
  const className = `${base} ${activeClass}`;

  if (to) {
    return (
      <Link to={to} className={className}>
        <Icon size={18} />
        {label && <span className="truncate">{label}</span>}
      </Link>
    );
  }

  return (
    <button type="button" onClick={onClick} className={className}>
      <Icon size={18} />
      {label && <span className="truncate">{label}</span>}
    </button>
  );
}
