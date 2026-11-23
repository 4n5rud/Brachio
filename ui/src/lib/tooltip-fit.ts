/** How far to move a box sideways so it lies inside the window with `margin` to spare; 0 when it already does. */
export function shiftIntoWindow(box: { left: number; right: number }, windowWidth: number, margin = 4): number {
  if (box.right - box.left > windowWidth - 2 * margin) return margin - box.left;
  if (box.left < margin) return margin - box.left;
  if (box.right > windowWidth - margin) return windowWidth - margin - box.right;
  return 0;
}
