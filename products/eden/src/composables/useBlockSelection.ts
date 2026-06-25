// Edge-zone autoscroll helper for the retired Eden block-selection experiment.
const AUTO_SCROLL_EDGE_PX = 48;
const AUTO_SCROLL_MAX_SPEED_PX = 16;

export function computeBlockSelectionAutoScrollDelta(
  clientY: number,
  container: HTMLElement,
): number {
  const rect = container.getBoundingClientRect();
  const fromTop = clientY - rect.top;
  const fromBottom = rect.bottom - clientY;
  if (fromTop < AUTO_SCROLL_EDGE_PX && fromTop < fromBottom) {
    const depth = Math.max(0, AUTO_SCROLL_EDGE_PX - fromTop);
    const intensity = Math.min(1, depth / AUTO_SCROLL_EDGE_PX);
    return -Math.ceil(intensity * AUTO_SCROLL_MAX_SPEED_PX);
  }
  if (fromBottom < AUTO_SCROLL_EDGE_PX) {
    const depth = Math.max(0, AUTO_SCROLL_EDGE_PX - fromBottom);
    const intensity = Math.min(1, depth / AUTO_SCROLL_EDGE_PX);
    return Math.ceil(intensity * AUTO_SCROLL_MAX_SPEED_PX);
  }
  return 0;
}
