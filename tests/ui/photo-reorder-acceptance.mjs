// Drag and keyboard reordering of the filmstrip with undo/redo. Synthetic catalog only.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin photo reorder');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/B.jpg','/synthetic/C.jpg']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===3&&!window.__qa.importControl.activeImports);
const catalog=()=>page.evaluate(()=>window.__qa.catalog.photos.map(p=>p.id));
const strip=()=>page.evaluate(()=>[...document.querySelectorAll('.filmstrip .thumbnail')].map(t=>t.dataset.photoId));
const [a,b,c]=await catalog();

// Drag the first photo past the last one.
const box=async n=>page.evaluate(n=>{const r=document.querySelectorAll('.filmstrip .thumbnail')[n].getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,right:r.right};},n);
const from=await box(0),to=await box(2);
await page.mouse.move(from.x,from.y);await page.mouse.down();
await page.mouse.move(from.x+20,from.y,{steps:3});
await page.mouse.move(to.right-4,to.y,{steps:6});
assert.equal(await page.evaluate(()=>document.querySelectorAll('.thumbnail.drop-after').length),1,'Shows where the photo will land');
await page.mouse.up();
await page.waitForFunction(id=>window.__qa.catalog.photos.at(-1).id===id,a);
assert.deepEqual(await catalog(),[b,c,a]);
assert.deepEqual(await strip(),[b,c,a]);
assert.equal(await page.evaluate(()=>document.querySelector('.thumbnail.active').dataset.photoId),a,'Dragging does not change the selection');
console.log('PASS: dragging a thumbnail reorders the saved catalogue');

await page.click('.header-undo');
await page.waitForFunction(id=>window.__qa.catalog.photos[0].id===id,a);
assert.deepEqual(await strip(),[a,b,c]);
await page.click('.header-redo');
await page.waitForFunction(id=>window.__qa.catalog.photos.at(-1).id===id,a);
console.log('PASS: undo and redo restore the order');

// Keyboard: Alt+Left moves the selected photo one place.
await page.click('.filmstrip .thumbnail >> nth=2');
await page.keyboard.press('Alt+ArrowLeft');
await page.waitForFunction(id=>window.__qa.catalog.photos[1].id===id,a);
assert.deepEqual(await strip(),[b,a,c]);
console.log('PASS: Alt+Left moves the selected photo');

// A plain click still selects instead of dragging.
await page.click('.filmstrip .thumbnail >> nth=2');
assert.equal(await page.evaluate(()=>document.querySelector('.thumbnail.active').dataset.photoId),c);
assert.deepEqual(await catalog(),[b,a,c]);
console.log('PASS: clicking selects without moving');
await task.finish();
