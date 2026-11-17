import assert from 'node:assert/strict';
import {test} from 'node:test';
import {singlePhotoSelection,allPhotoSelection,clearPhotoSelection,restoredPhotoSelection,selectCatalogPhoto,contextCatalogPhoto,selectionAfterRemoval} from '../src/lib/photo-selection.ts';

const order=['a','b','c','d','e','f'];
test('Undo restores the prior selection and maps reimported IDs',()=>{
  const before=selectCatalogPhoto(order,singlePhotoSelection('b'),'d',{range:true});
  assert.deepEqual(restoredPhotoSelection(order,before),before);
  assert.deepEqual(restoredPhotoSelection(['a','new-b','c','d','e','f','added'],before,{b:'new-b'}),{activeId:'d',ids:['new-b','c','d'],anchorId:'new-b'});
  assert.deepEqual(restoredPhotoSelection(order,clearPhotoSelection(before)).ids,[]);
  assert.deepEqual(restoredPhotoSelection([],before),singlePhotoSelection());
});
test('Select all and clear preserve the current preview without mutating prior state',()=>{
  const initial=singlePhotoSelection('c');
  const all=allPhotoSelection(order,initial);
  assert.deepEqual(all.ids,order);assert.equal(all.activeId,'c');assert.equal(all.anchorId,'c');
  const cleared=clearPhotoSelection(all);
  assert.deepEqual(cleared.ids,[]);assert.equal(cleared.activeId,'c');
  assert.deepEqual(selectCatalogPhoto(order,cleared,'f',{range:true}).ids,['c','d','e','f']);
  assert.deepEqual(initial,singlePhotoSelection('c'));
  assert.deepEqual(allPhotoSelection([],initial),singlePhotoSelection());
  assert.deepEqual(allPhotoSelection(order,singlePhotoSelection('missing')),allPhotoSelection(order,singlePhotoSelection('a')));
});
test('Command/Ctrl toggles individual photos, ordinary click replaces the group',()=>{
  const initial=singlePhotoSelection('b');
  let selection=selectCatalogPhoto(order,initial,'e',{additive:true});
  assert.deepEqual(selection.ids,['b','e']);assert.equal(selection.activeId,'e');
  selection=selectCatalogPhoto(order,selection,'e',{additive:true});
  assert.deepEqual(selection.ids,['b']);assert.equal(selection.activeId,'b');
  selection=selectCatalogPhoto(order,selection,'b',{additive:true});
  assert.deepEqual(selection.ids,[]);
  assert.deepEqual(selectCatalogPhoto(order,selection,'c'),singlePhotoSelection('c'));
  assert.deepEqual(initial,singlePhotoSelection('b'),'Inputs must not be mutated');
});
test('Shift ranges expand and contract from a stable anchor in both directions',()=>{
  let selection=singlePhotoSelection('c');
  selection=selectCatalogPhoto(order,selection,'f',{range:true});
  assert.deepEqual(selection.ids,['c','d','e','f']);
  selection=selectCatalogPhoto(order,selection,'d',{range:true});
  assert.deepEqual(selection.ids,['c','d']);
  selection=selectCatalogPhoto(order,selection,'a',{range:true});
  assert.deepEqual(selection.ids,['a','b','c']);assert.equal(selection.anchorId,'c');
});
test('Command+Shift adds a range and context click preserves or replaces membership',()=>{
  let selection=selectCatalogPhoto(order,singlePhotoSelection('a'),'d',{additive:true});
  selection=selectCatalogPhoto(order,selection,'f',{range:true,additive:true});
  assert.deepEqual(selection.ids,['a','d','e','f']);
  const context=contextCatalogPhoto(order,selection,'d');
  assert.deepEqual(context.ids,selection.ids);assert.equal(context.activeId,'d');
  assert.equal(context.anchorId,selection.anchorId);
  assert.deepEqual(contextCatalogPhoto(order,context,'c'),singlePhotoSelection('c'));
  assert.equal(selectCatalogPhoto(order,selection,'missing'),selection);
});
test('Removal selects the nearest surviving photo, or preserves surviving selection',()=>{
  const selection=selectCatalogPhoto(order,singlePhotoSelection('b'),'d',{range:true});
  assert.deepEqual(selectionAfterRemoval(order,selection,['b','c','d']),singlePhotoSelection('e'));
  assert.deepEqual(selectionAfterRemoval(order,selection,['d']).ids,['b','c']);
  assert.equal(selectionAfterRemoval(order,selection,['d']).activeId,'c');
  assert.deepEqual(selectionAfterRemoval(order,singlePhotoSelection('f'),['f']),singlePhotoSelection('e'));
  assert.deepEqual(selectionAfterRemoval(order,selection,order),singlePhotoSelection());
});
