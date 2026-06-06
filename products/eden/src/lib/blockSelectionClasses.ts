interface BlockSelectionClassState {
  hasSelection: boolean;
  isDragging: boolean;
}

export function applyBlockSelectionClasses(
  element: HTMLElement,
  state: BlockSelectionClassState,
): void {
  element.classList.toggle("kepler-block-select-active", state.hasSelection || state.isDragging);
  element.classList.toggle("kepler-block-drag-active", state.isDragging);
}
