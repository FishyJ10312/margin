export interface PhotoSelection {
  activeId: string;
  ids: string[];
  anchorId: string;
}

export function singlePhotoSelection(id = ''): PhotoSelection {
  return {activeId: id, ids: id ? [id] : [], anchorId: id};
}

export function allPhotoSelection(order: string[], selection: PhotoSelection): PhotoSelection {
  if (!order.length) return singlePhotoSelection();
  const activeId = order.includes(selection.activeId) ? selection.activeId : order[0];
  return {activeId, ids: [...order], anchorId: order.includes(selection.anchorId) ? selection.anchorId : activeId};
}

export function clearPhotoSelection(selection: PhotoSelection): PhotoSelection {
  return {...selection, ids: [], anchorId: selection.activeId};
}

export function restoredPhotoSelection(order:string[], before:PhotoSelection, idMap:Record<string,string>={}):PhotoSelection {
  const map=(id:string)=>idMap[id]??id;
  const wanted=new Set(before.ids.map(map));
  const ids=order.filter(id=>wanted.has(id));
  const activeId=order.includes(map(before.activeId))?map(before.activeId):ids[0]??order[0]??'';
  return {activeId,ids,anchorId:order.includes(map(before.anchorId))?map(before.anchorId):activeId};
}

/** Catalog order, rather than click order, defines ranges and removal targets. */
export function selectCatalogPhoto(
  order: string[], selection: PhotoSelection, id: string,
  modifiers: {additive?: boolean; range?: boolean} = {},
): PhotoSelection {
  const index = order.indexOf(id);
  if (index < 0) return selection;
  if (modifiers.range) {
    const anchorId = order.includes(selection.anchorId) ? selection.anchorId
      : order.includes(selection.activeId) ? selection.activeId : id;
    const anchor = order.indexOf(anchorId);
    const range = order.slice(Math.min(anchor, index), Math.max(anchor, index) + 1);
    const ids = new Set(modifiers.additive ? [...selection.ids, ...range] : range);
    return {activeId: id, ids: order.filter(value => ids.has(value)), anchorId};
  }
  if (modifiers.additive) {
    const ids = new Set(selection.ids);
    if (ids.has(id)) ids.delete(id); else ids.add(id);
    const selected = order.filter(value => ids.has(value));
    const activeId = ids.has(id) ? id : ids.has(selection.activeId) ? selection.activeId
      : order.slice(index + 1).find(value => ids.has(value)) ?? selected[selected.length - 1] ?? id;
    return {activeId, ids: selected, anchorId: id};
  }
  return singlePhotoSelection(id);
}

/** A context click inside the selection must keep the whole selection intact. */
export function contextCatalogPhoto(order: string[], selection: PhotoSelection, id: string): PhotoSelection {
  if (!order.includes(id)) return selection;
  return selection.ids.includes(id) ? {...selection, activeId: id} : singlePhotoSelection(id);
}

export function selectionAfterRemoval(order: string[], selection: PhotoSelection, removedIds: string[]): PhotoSelection {
  const removed = new Set(removedIds);
  const remaining = order.filter(id => !removed.has(id));
  if (!remaining.length) return singlePhotoSelection();
  const remainingIds = new Set(remaining);
  const indices = new Map(order.map((id, index) => [id, index]));
  const previousIndex = order.indexOf(selection.activeId);
  const next = remaining.find(id => indices.get(id)! >= previousIndex) ?? remaining[remaining.length - 1];
  const ids = selection.ids.filter(id => remainingIds.has(id));
  if (!ids.length) return singlePhotoSelection(remainingIds.has(selection.activeId) ? selection.activeId : next);
  const activeId = ids.includes(selection.activeId) ? selection.activeId
    : ids.find(id => indices.get(id)! >= previousIndex) ?? ids[ids.length - 1];
  return {activeId, ids, anchorId: remainingIds.has(selection.anchorId) ? selection.anchorId : activeId};
}
