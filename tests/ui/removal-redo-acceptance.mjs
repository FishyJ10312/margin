// Real UI and keyboard interactions, isolated synthetic catalog and native-call mocks.
import assert from 'node:assert/strict';
const screenshotDirectory=(await import('node:path')).join((await import('node:os')).tmpdir(),'margin-ui-acceptance');
await (await import('node:fs/promises')).mkdir(screenshotDirectory,{recursive:true});
const task=await taskSpace('Margin removal redo');console.log({spaceId:task.spaceId});const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png','/synthetic/four.png','/synthetic/five.png','/synthetic/six.png']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===6&&!window.__qa.importControl.activeImports);
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const state=()=>page.evaluate(()=>({names:[...document.querySelectorAll('.thumbnail')].map(e=>e.title),selected:[...document.querySelectorAll('.thumbnail.selected')].map(e=>e.title),active:document.querySelector('.thumbnail.active')?.title}));
const calls=cmd=>page.evaluate(cmd=>window.__qa.requests.filter(r=>r.cmd===cmd).length,cmd);
async function click(n,keys=[]){for(const k of keys)await page.keyboard.down(k);try{await page.click(thumb(n));}finally{for(const k of [...keys].reverse())await page.keyboard.up(k);}}
async function count(n){await page.waitForFunction(n=>document.querySelectorAll('.thumbnail').length===n,n);}
async function remove(){await page.keyboard.press('Backspace');await page.waitForSelector('.photo-removal-dialog[open]');await page.click('.confirm-photo-removal');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});}
async function undo(n){await page.keyboard.press('Meta+z');await count(n);}
async function redo(n){await page.keyboard.press('Meta+Shift+z');await count(n);}
const original=(await state()).names;

await click(1);await click(4,['Shift']);const first=await state();await remove();await count(2);
await click(1);const second=await state();await remove();await count(1);
await undo(2);assert.deepEqual(await state(),second);await undo(6);assert.deepEqual(await state(),first);
await redo(2);assert.deepEqual((await state()).names,[original[0],original[5]]);
await redo(1);assert.deepEqual((await state()).names,[original[0]]);
await undo(2);await undo(6);assert.deepEqual((await state()).names,original);
await click(2,['Shift']);assert.deepEqual((await state()).selected,original.slice(1,3));
console.log('PASS: consecutive Command+Shift+Z replays the same operations in order; repeated undo restores order, selection and Shift anchor.');

// Redo targets the original operation, even if selection has changed afterwards.
await click(0);const beforeRedo=await state();
await page.fill('#frame-hex','#DED5C4');const protectedCalls=await calls('redo_photo_removal');
await page.keyboard.press('Meta+Shift+z');assert.equal(await calls('redo_photo_removal'),protectedCalls);
await page.fill('#frame-hex','#DED5C4');await page.focus(thumb(0));
await page.evaluate(()=>window.__qa.redoControl.fail=true);await page.keyboard.press('Meta+Shift+z');
await page.waitForFunction(()=>document.querySelector('.catalog-undo [role="alert"]')?.textContent.includes('Fixture cannot redo'));
assert.deepEqual(await state(),beforeRedo);
await page.evaluate(()=>{window.__qa.redoControl.fail=false;window.__qa.redoControl.hold=true;});await page.click('.redo-photo-removal');
await page.waitForFunction(()=>window.__qa.redoControl.pending.length===1);const inFlight=await calls('redo_photo_removal');
await page.keyboard.press('Meta+Shift+z');await page.evaluate(()=>document.querySelector('.redo-photo-removal').click());assert.equal(await calls('redo_photo_removal'),inFlight);
await page.evaluate(()=>{window.__qa.redoControl.hold=false;window.__qa.redoControl.pending.shift()();});await count(2);
assert.deepEqual((await state()).names,[original[0],original[5]]);assert.deepEqual((await state()).selected,[original[0]]);
assert.match(await page.evaluate(()=>document.querySelector('.export-button').textContent),/1/);
await undo(6);assert.deepEqual(await state(),beforeRedo);assert.equal(await page.evaluate(()=>document.querySelector('#frame-hex').value),'#DED5C4');
console.log('PASS: redo uses recorded targets, keeps current settings, restores the pre-redo selection on undo, and retries failed saves without duplicate dispatch.');

// A genuinely new removal branches history. A cancelled confirmation does not.
await click(3);await page.keyboard.press('Backspace');await page.waitForSelector('.photo-removal-dialog[open]');
const modalCalls=await calls('redo_photo_removal');await page.keyboard.press('Meta+Shift+z');assert.equal(await calls('redo_photo_removal'),modalCalls);
await page.keyboard.press('Escape');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});
assert.ok(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')));
await remove();await count(5);assert.equal(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')),false);
await undo(6);await page.evaluate(()=>window.__qa.drop(['/synthetic/photo.png','/synthetic/ignored.txt']));
await page.waitForFunction(()=>!window.__qa.importControl.activeImports&&document.querySelector('.import-feedback')?.textContent.includes('0'));
assert.ok(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')));
await page.evaluate(()=>window.__qa.drop(['/synthetic/later.png']));await count(7);
assert.equal(await page.evaluate(()=>!!document.querySelector('.redo-photo-removal')),false);
console.log('PASS: cancel and duplicate/unsupported import keep redo; a successful new removal or new import clears the old redo branch.');

await page.focus(thumb(0));await page.keyboard.press('Meta+a');await remove();await count(0);
await page.click('.undo-photo-removal');await count(7);const all=await state();
await page.click('.redo-photo-removal');await count(0);assert.ok(await page.evaluate(()=>!!document.querySelector('.undo-photo-removal')));
await page.keyboard.press('Meta+z');await count(7);assert.deepEqual(await state(),all);
console.log('PASS: all-photo redo reaches the empty screen and remains undoable with all seven selected.');

await page.evaluate(()=>window.__qa.emitBatch({id:'redo-running',revision:1,status:'running',outputDirectory:'/synthetic/exports',items:[]}));
const guarded=await calls('redo_photo_removal');await page.focus(thumb(0));await page.keyboard.press('Meta+Shift+z');
assert.equal(await calls('redo_photo_removal'),guarded);assert.equal(await page.evaluate(()=>document.querySelector('.redo-photo-removal').disabled),true);
await page.evaluate(()=>window.__qa.emitBatch({id:'redo-running',revision:2,status:'completed',outputDirectory:'/synthetic/exports',items:[]}));
await page.click('#inspector-watermark');await page.focus('#font-search');await page.keyboard.press('Meta+Shift+z');assert.equal(await calls('redo_photo_removal'),guarded);
await page.click('#inspector-frame');await page.focus(thumb(0));
await page.evaluate(()=>document.activeElement.dispatchEvent(new KeyboardEvent('keydown',{key:'Z',metaKey:true,shiftKey:true,isComposing:true,bubbles:true})));
assert.equal(await calls('redo_photo_removal'),guarded);
await page.keyboard.press('Shift+F10');await page.waitForSelector('.photo-context-menu');await page.keyboard.press('Meta+Shift+z');assert.equal(await calls('redo_photo_removal'),guarded);await page.keyboard.press('Escape');
await page.keyboard.press('Control+Shift+z');await count(0);await page.keyboard.press('Control+z');await count(7);
console.log('PASS: export, font search, IME, dialogs and context menu guard redo; Ctrl+Shift+Z also replays correctly.');

// Two available history actions must fit the supported compact window in Chinese.
await click(1);await remove();await click(1);await remove();await undo(6);
await page.selectOption('.language-select','zh-CN');await page.cdp('Emulation.setDeviceMetricsOverride',{width:900,height:650,deviceScaleFactor:1,mobile:false});
assert.match(await page.evaluate(()=>document.querySelector('.redo-photo-removal').textContent),/重做移除/);
assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth&&[...document.querySelectorAll('.catalog-history-actions button')].every(e=>{const r=e.getBoundingClientRect();return r.height>=24&&r.left>=0&&r.right<=innerWidth;})));
await page.screenshot({path:screenshotDirectory+'/redo-900x650.png'});
console.log('PASS: undo and redo actions fit the compact Chinese UI. All fixture redo checks passed.');
await task.finish({keep:[]});
