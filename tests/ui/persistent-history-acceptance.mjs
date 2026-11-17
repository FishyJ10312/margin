// UI reload coverage uses a synthetic, namespaced mock journal. Real process restarts
// and atomic disk persistence are checked separately with Rust and the packaged Mac app.
import assert from 'node:assert/strict';
const screenshotDirectory=(await import('node:path')).join((await import('node:os')).tmpdir(),'margin-ui-acceptance');
await (await import('node:fs/promises')).mkdir(screenshotDirectory,{recursive:true});
const task=await taskSpace('Margin persistent history');console.log({spaceId:task.spaceId});const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html?history-persistence='+Date.now());
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png','/synthetic/four.png','/synthetic/five.png','/synthetic/six.png']));
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const state=()=>page.evaluate(()=>({names:[...document.querySelectorAll('.thumbnail')].map(e=>e.title),selected:[...document.querySelectorAll('.thumbnail.selected')].map(e=>e.title),active:document.querySelector('.thumbnail.active')?.title}));
async function count(n){await page.waitForFunction(n=>document.querySelectorAll('.thumbnail').length===n,n);}
async function click(n,keys=[]){for(const key of keys)await page.keyboard.down(key);try{await page.click(thumb(n));}finally{for(const key of [...keys].reverse())await page.keyboard.up(key);}}
async function selectionSaved(){await page.waitForFunction(()=>{const c=window.__qa.catalog;return !window.__qa.selectionControl.active&&JSON.stringify(c.photoSelection.ids)===JSON.stringify([...document.querySelectorAll('.thumbnail.selected')].map(e=>e.dataset.photoId))&&c.photoSelection.activeId===(document.querySelector('.thumbnail.active')?.dataset.photoId??'');});}
async function reload(n){await page.reload();await page.waitForFunction(()=>window.__qa?.dropReady&&!!document.querySelector('.library-summary'));await count(n);}
async function remove(){await page.keyboard.press('Backspace');await page.waitForSelector('.photo-removal-dialog[open]');await page.click('.confirm-photo-removal');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});}
async function undo(n){await page.keyboard.press('Meta+z');await count(n);}
async function redo(n){await page.keyboard.press('Meta+Shift+z');await count(n);}
await count(6);const original=(await state()).names;
await page.fill('#frame-hex','#F3ECDD');await page.focus(thumb(0));await click(1);await click(4,['Shift']);const range=await state();
await remove();await count(2);const reduced=await state();await reload(2);assert.deepEqual(await state(),reduced);
await page.focus(thumb(0));await undo(6);assert.deepEqual(await state(),range);
assert.equal(await page.evaluate(()=>document.querySelector('#frame-hex').value),'#F3ECDD');
await reload(6);assert.deepEqual(await state(),range);assert.ok(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')));
await click(2,['Shift']);const rangeBeforeRedo=await state();assert.deepEqual(rangeBeforeRedo.selected,original.slice(1,3));
await selectionSaved();await reload(6);assert.deepEqual(await state(),rangeBeforeRedo);
await page.focus(thumb(0));await redo(2);assert.deepEqual((await state()).names,[original[0],original[5]]);
await reload(2);await page.focus(thumb(0));await undo(6);assert.deepEqual(await state(),rangeBeforeRedo);
console.log('PASS: reload hydrates undo/redo, original order, active photo, multi-selection and Shift anchor; settings and changed pre-redo selection are preserved.');

await page.focus(thumb(0));await redo(2);await click(1);await remove();await count(1);await undo(2);
await reload(2);assert.ok(await page.evaluate(()=>!!document.querySelector('.undo-photo-removal')&&!!document.querySelector('.redo-photo-removal')));
await page.focus(thumb(0));await redo(1);await reload(1);await page.focus(thumb(0));await undo(2);await undo(6);
await reload(6);assert.deepEqual((await state()).names,original);
await click(0);await remove();await count(5);await reload(5);
assert.equal(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')),false);
await page.focus(thumb(0));await undo(6);await page.evaluate(()=>window.__qa.drop(['/synthetic/new-photo.png']));await count(7);await reload(7);
assert.equal(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')),false);
console.log('PASS: both stacks coexist across reloads, replay chronologically, and a successful new removal/import invalidates redo durably.');

await click(0);await click(3,['Meta']);const sparse=await state();await selectionSaved();await reload(7);assert.deepEqual(await state(),sparse);
await page.focus(thumb(3));await page.keyboard.press('Escape');const none=await state();await selectionSaved();await reload(7);assert.deepEqual(await state(),none);
assert.equal(await page.evaluate(()=>document.querySelector('.export-button').disabled),true);
await page.focus(thumb(0));await page.keyboard.press('Meta+a');await remove();await count(0);await reload(0);
await page.click('.undo-photo-removal');await count(7);const all=await state();assert.equal(all.selected.length,7);
await reload(7);await page.click('.redo-photo-removal');await count(0);await reload(0);await page.click('.undo-photo-removal');await count(7);assert.deepEqual(await state(),all);
console.log('PASS: sparse and empty selection survive reloads, and an empty catalog can undo/redo all-photo removal after repeated reloads.');

await click(1);await click(3,['Shift']);const failedRange=await state();await remove();await count(4);await reload(4);
await page.evaluate(()=>window.__qa.undoControl.fail=true);await page.focus(thumb(0));await page.keyboard.press('Meta+z');
await page.waitForFunction(()=>document.querySelector('.catalog-undo [role="alert"]')?.textContent.includes('Fixture cannot restore'));
await reload(4);await page.focus(thumb(0));await undo(7);assert.deepEqual(await state(),failedRange);
await reload(7);await page.evaluate(()=>window.__qa.redoControl.fail=true);await page.focus(thumb(0));await page.keyboard.press('Meta+Shift+z');
await page.waitForFunction(()=>document.querySelector('.catalog-undo [role="alert"]')?.textContent.includes('Fixture cannot redo'));
await reload(7);await page.focus(thumb(0));await redo(4);await undo(7);assert.deepEqual(await state(),failedRange);
console.log('PASS: failed undo/redo does not consume the durable receipt; reload can retry successfully.');

await page.evaluate(()=>window.__qa.selectionControl.hold=true);await click(0);await page.waitForFunction(()=>window.__qa.selectionControl.pending.length===1);
await click(1);await click(2);await click(3);assert.equal(await page.evaluate(()=>window.__qa.selectionControl.pending.length),1);
await remove();await count(6);const latest=await state();
await page.evaluate(()=>{window.__qa.selectionControl.hold=false;window.__qa.selectionControl.pending.shift()();});await selectionSaved();await reload(6);assert.deepEqual(await state(),latest);
assert.equal(await page.evaluate(()=>window.__qa.selectionControl.maxActive),0); // New mock process has not needed another selection write.
await page.focus(thumb(0));await undo(7);assert.deepEqual((await state()).selected,[original[3]]);
console.log('PASS: selection writes coalesce; a stale in-flight selection cannot overwrite a later removal or the state restored on reload.');

await page.selectOption('.language-select','zh-CN');await page.cdp('Emulation.setDeviceMetricsOverride',{width:900,height:650,deviceScaleFactor:1,mobile:false});
assert.match(await page.evaluate(()=>document.querySelector('.undo-photo-removal')?.title??document.querySelector('.redo-photo-removal').title),/重启|重新执行/);
assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
await page.screenshot({path:screenshotDirectory+'/saved-history-900x650.png'});
console.log('All persistent history UI checks passed.');await task.finish({keep:[]});
