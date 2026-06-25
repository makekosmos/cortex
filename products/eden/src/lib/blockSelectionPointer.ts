function resolvePointerTargetElement(target: EventTarget | null): HTMLElement | null {
  if (target instanceof HTMLElement) return target;
  if (target instanceof Node && target.parentElement instanceof HTMLElement) {
    return target.parentElement;
  }
  return null;
}

export function shouldStartBlockSelectionTracking(
  target: EventTarget | null,
  contentArea: HTMLElement,
): boolean {
  const element = resolvePointerTargetElement(target);
  if (!element) return false;
  if (!contentArea.contains(element)) return false;

  const proseMirror = element.closest(".ProseMirror");
  if (element.closest("button, input, textarea, select, [role=button], [role=checkbox]")) {
    return false;
  }
  if (!proseMirror) return true;

  // Regression: 2026-05-28. Text drag inside ProseMirror must stay native
  // selection; otherwise block-selection hijacks it and can auto-scroll.
  // См. postmortems.md § 2026-05-23 Eden UPDATE 2026-05-28.
  return element === proseMirror;
}
