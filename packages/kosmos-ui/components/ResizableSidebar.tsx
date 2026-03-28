import { useState, useEffect, useCallback, useRef, type ReactNode, type MouseEvent as ReactMouseEvent } from 'react';
import './sidebar.css';

export interface SidebarConfig {
  width: number;
  collapsed: boolean;
}

export interface ResizableSidebarProps {
  /** Sidebar content when expanded — render prop receiving toggle */
  children: (toggle: () => void) => ReactNode;
  /** Default width in pixels */
  defaultWidth?: number;
  /** Minimum width before collapse threshold */
  minWidth?: number;
  /** Maximum width */
  maxWidth?: number;
  /** Width below which sidebar auto-collapses during drag */
  collapseThreshold?: number;
  /** Keyboard shortcut to toggle (e.g., "meta+/") */
  toggleShortcut?: string;
  /** CSS class for the sidebar wrapper */
  className?: string;
  /** Callback when config changes (for persistence) */
  onConfigChange?: (config: SidebarConfig) => void;
  /** Initial config (loaded from persistence) */
  initialConfig?: Partial<SidebarConfig>;
  /** Content to show when collapsed — render prop receiving toggle */
  collapsedContent?: (toggle: () => void) => ReactNode;
  /** Drag region for Electron titlebar */
  dragRegion?: boolean;
}

export function ResizableSidebar({
  children,
  defaultWidth = 200,
  minWidth = 160,
  maxWidth = 320,
  collapseThreshold = 60,
  toggleShortcut,
  className,
  onConfigChange,
  initialConfig,
  collapsedContent,
  dragRegion = false,
}: ResizableSidebarProps) {
  const [width, setWidth] = useState(initialConfig?.width ?? defaultWidth);
  const [collapsed, setCollapsed] = useState(initialConfig?.collapsed ?? false);
  const [isResizing, setIsResizing] = useState(false);
  const [animating, setAnimating] = useState(false);

  const resizeRef = useRef<number | null>(null);
  const animTimerRef = useRef<number | null>(null);
  const isAnimatingRef = useRef(false);
  const configRef = useRef<SidebarConfig>({ width, collapsed });

  useEffect(() => {
    configRef.current = { width, collapsed };
  }, [width, collapsed]);

  const notifyConfigChange = useCallback((config: SidebarConfig) => {
    onConfigChange?.(config);
  }, [onConfigChange]);

  const startAnimation = useCallback(() => {
    if (animTimerRef.current) {
      window.clearTimeout(animTimerRef.current);
    }
    isAnimatingRef.current = true;
    setAnimating(true);
    animTimerRef.current = window.setTimeout(() => {
      isAnimatingRef.current = false;
      setAnimating(false);
      animTimerRef.current = null;
    }, 220);
  }, []);

  const toggle = useCallback(() => {
    startAnimation();
    setCollapsed((prev) => {
      const next = !prev;
      const config = { width: configRef.current.width, collapsed: next };
      notifyConfigChange(config);
      return next;
    });
  }, [startAnimation, notifyConfigChange]);

  // Keyboard shortcut
  useEffect(() => {
    if (!toggleShortcut) return;

    const parts = toggleShortcut.toLowerCase().split('+');
    const key = parts[parts.length - 1];
    const needsMeta = parts.includes('meta');
    const needsCtrl = parts.includes('ctrl');
    const needsShift = parts.includes('shift');
    const needsAlt = parts.includes('alt');

    const handler = (e: KeyboardEvent) => {
      if (needsMeta && !e.metaKey) return;
      if (needsCtrl && !e.ctrlKey) return;
      if (needsShift && !e.shiftKey) return;
      if (needsAlt && !e.altKey) return;
      if (e.key !== key && e.key.toLowerCase() !== key) return;

      e.preventDefault();
      toggle();
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [toggleShortcut, toggle]);

  // Resize mouse handlers
  const handleResizeStart = useCallback((e: ReactMouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
    document.body.classList.add('sidebar-resizing');
  }, []);

  const handleResizeMove = useCallback((e: MouseEvent) => {
    if (isAnimatingRef.current) return;

    if (resizeRef.current) {
      cancelAnimationFrame(resizeRef.current);
    }

    resizeRef.current = requestAnimationFrame(() => {
      if (isAnimatingRef.current) return;

      const newWidth = e.clientX;

      if (newWidth <= collapseThreshold) {
        if (!configRef.current.collapsed) {
          startAnimation();
          setCollapsed(true);
          configRef.current = { ...configRef.current, collapsed: true };
        }
        return;
      }

      if (configRef.current.collapsed) {
        startAnimation();
        setCollapsed(false);
        setWidth(minWidth);
        configRef.current = { width: minWidth, collapsed: false };
        return;
      }

      const clamped = Math.max(minWidth, Math.min(maxWidth, newWidth));
      setWidth(clamped);
      configRef.current = { ...configRef.current, width: clamped };
    });
  }, [collapseThreshold, minWidth, maxWidth, startAnimation]);

  const handleResizeEnd = useCallback(() => {
    if (resizeRef.current) {
      cancelAnimationFrame(resizeRef.current);
      resizeRef.current = null;
    }
    setIsResizing(false);
    document.body.classList.remove('sidebar-resizing');
    notifyConfigChange(configRef.current);
  }, [notifyConfigChange]);

  // Attach/detach global mouse listeners during resize
  useEffect(() => {
    if (!isResizing) return;

    window.addEventListener('mousemove', handleResizeMove);
    window.addEventListener('mouseup', handleResizeEnd);

    return () => {
      window.removeEventListener('mousemove', handleResizeMove);
      window.removeEventListener('mouseup', handleResizeEnd);
    };
  }, [isResizing, handleResizeMove, handleResizeEnd]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (animTimerRef.current) window.clearTimeout(animTimerRef.current);
      if (resizeRef.current) cancelAnimationFrame(resizeRef.current);
      document.body.classList.remove('sidebar-resizing');
    };
  }, []);

  // When collapsed and collapsed content is provided, render it directly
  if (collapsed && collapsedContent) {
    return <>{collapsedContent(toggle)}</>;
  }

  // When collapsed without collapsed content, render zero-width wrapper
  if (collapsed) {
    return (
      <div
        className={[
          'kosmos-sidebar-wrapper',
          'collapsed',
          animating ? 'animating' : '',
          className ?? '',
        ].filter(Boolean).join(' ')}
        style={{ width: 0 }}
      />
    );
  }

  const wrapperClasses = [
    'kosmos-sidebar-wrapper',
    animating ? 'animating' : '',
    isResizing ? 'is-resizing' : '',
    className ?? '',
  ].filter(Boolean).join(' ');

  return (
    <div className={wrapperClasses} style={{ width }}>
      {dragRegion && (
        <div
          style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
          className="kosmos-sidebar-drag-region"
        />
      )}
      <div className="kosmos-sidebar-content" style={{ width, minWidth: width }}>
        {children(toggle)}
      </div>
      <div className="kosmos-sidebar-resize-handle" onMouseDown={handleResizeStart}>
        <div className="kosmos-resize-handle-line" />
      </div>
    </div>
  );
}
