/** Move one item; returns a new array. No-op when `from === to` or out of range. */
export function moveArrayItem<T>(
  items: readonly T[],
  from: number,
  to: number,
): T[] {
  if (
    from === to ||
    from < 0 ||
    from >= items.length ||
    to < 0 ||
    to >= items.length
  ) {
    return [...items];
  }
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

/** Update the active tab index after `moveArrayItem(from, to)`. */
export function remapIndexAfterMove(
  active: number,
  from: number,
  to: number,
): number {
  if (active < 0) return active;
  if (active === from) return to;
  if (from < to) {
    if (active > from && active <= to) return active - 1;
  } else if (from > to) {
    if (active >= to && active < from) return active + 1;
  }
  return active;
}

/**
 * `dropSlot` is an insertion point in the tab strip: `0` = before the first tab,
 * `length` = after the last. Returns the target index in the reordered array.
 */
export function dropSlotToMoveIndex(
  from: number,
  dropSlot: number,
  length: number,
): number {
  if (length <= 0) return 0;
  const slot = Math.max(0, Math.min(dropSlot, length));
  let insertAt = slot;
  if (from < slot) insertAt = slot - 1;
  return Math.max(0, Math.min(length - 1, insertAt));
}
