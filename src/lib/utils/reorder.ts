export type DropPosition = "before" | "after";

/** Deplace l'element `from` avant/apres l'element `to` et retourne une NOUVELLE liste. */
export function reorder<T>(list: T[], from: number, to: number, pos: DropPosition): T[] {
  if (from === to) return list;
  const next = [...list];
  const [moved] = next.splice(from, 1);
  let target = to > from ? to - 1 : to;
  if (pos === "after") target += 1;
  next.splice(target, 0, moved);
  return next;
}

/** Groupe une liste par cle — remplace les Map reconstruites a la main. */
export function groupBy<T>(list: T[], keyFn: (item: T) => string): { key: string; items: T[] }[] {
  const map = new Map<string, T[]>();
  for (const item of list) {
    const k = keyFn(item);
    if (!map.has(k)) map.set(k, []);
    map.get(k)!.push(item);
  }
  return Array.from(map.entries()).map(([key, items]) => ({ key, items }));
}
