// Real UI + synthetic catalog. No model calls or user photos.
import assert from 'node:assert/strict';
const screenshotDirectory=(await import('node:path')).join((await import('node:os')).tmpdir(),'margin-ui-acceptance');
await (await import('node:fs/promises')).mkdir(screenshotDirectory,{recursive:true});
const task=await taskSpace('Margin removal undo');console.log({spaceId:task.spaceId});const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png','/synthetic/four.png','/synthetic/five.png','/synthetic/six.png']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===6&&!window.__qa.importControl.activeImports);
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const state=()=>page.evaluate(()=>({names:[...document.querySelectorAll('.thumbnail')].map(e=>e.title),selected:[...document.querySelectorAll('.thumbnail.selected')].map(e=>e.title),active:document.querySelector('.thumbnail.active')?.title}));
const calls=cmd=>page.evaluate(cmd=>window.__qa.requests.filter(r=>r.cmd===cmd),cmd);
async function click(n,keys=[]){for(const k of keys)await page.keyboard.down(k);try{await page.click(thumb(n));}finally{for(const k of [...keys].reverse())await page.keyboard.up(k);}}
async function remove(){await page.keyboard.press('Backspace');await page.waitForSelector('.photo-removal-dialog[open]');await page.click('.confirm-photo-removal');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});}
async function undo(count){await page.keyboard.press('Meta+z');await page.waitForFunction(n=>document.querySelectorAll('.thumbnail').length===n,count);}
const original=(await state()).names;

await click(1);await click(4,['Shift']);const before=await state();await remove();
assert.deepEqual((await state()).names,[original[0],original[5]]);await page.waitForSelector('.undo-photo-removal');
await page.fill('#frame-hex','#DED5C4');const beforeUndo=(await calls('undo_photo_removal')).length;
await page.keyboard.press('Meta+z');assert.equal((await calls('undo_photo_removal')).length,beforeUndo,'Text undo must not restore photos');
await page.fill('#frame-hex','#DED5C4');await page.focus(thumb(0));await undo(6);
assert.deepEqual(await state(),before);assert.equal(await page.evaluate(()=>document.querySelector('#frame-hex').value),'#DED5C4');
await click(2,['Shift']);assert.deepEqual((await state()).selected,[original[1],original[2]]);
console.log('PASS: range removal undo restores order, active preview, selection and Shift anchor; current settings and text undo remain independent.');

await click(0);await click(3,['Meta']);const sparse=await state();await remove();
await page.evaluate(()=>window.__qa.undoControl.fail=true);await page.keyboard.press('Meta+z');
await page.waitForFunction(()=>document.querySelector('.catalog-undo [role="alert"]')?.textContent.includes('Fixture cannot restore catalog'));
assert.equal((await state()).names.length,4);
await page.evaluate(()=>{window.__qa.undoControl.fail=false;window.__qa.undoControl.hold=true;});await page.keyboard.press('Meta+z');
await page.waitForFunction(()=>window.__qa.undoControl.pending.length===1);const pending=(await calls('undo_photo_removal')).length;
await page.keyboard.press('Meta+z');await page.evaluate(()=>document.querySelector('.undo-photo-removal').click());assert.equal((await calls('undo_photo_removal')).length,pending);
await page.evaluate(()=>{window.__qa.undoControl.hold=false;window.__qa.undoControl.pending.shift()();});await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===6);
assert.deepEqual(await state(),sparse);console.log('PASS: failed undo retains retry history; in-flight undo cannot be submitted twice.');

await click(1);const first=await state();await remove();await click(1);await click(2,['Shift']);const second=await state();await remove();
await undo(5);assert.deepEqual(await state(),second);await undo(6);assert.deepEqual(await state(),first);
console.log('PASS: consecutive removals undo one operation at a time with each operation\'s original selection.');

await page.keyboard.press('Escape');await page.click('.clear-photos');await page.waitForSelector('.photo-removal-dialog[open]');await page.click('.confirm-photo-removal');
await page.waitForSelector('.empty-state');await page.click('.undo-photo-removal');await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===6);
assert.deepEqual((await state()).names,original);assert.deepEqual((await state()).selected,[]);
assert.equal(await page.evaluate(()=>document.querySelector('.export-button').disabled),true);
console.log('PASS: clear-all is undoable from the empty screen, including a prior empty selection.');

await click(1);await remove();await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/later.png']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===7&&!window.__qa.importControl.activeImports);
await page.focus(thumb(0));await page.keyboard.press('Control+z');
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail.selected').length===1&&document.querySelector('.thumbnail.selected').title==='two.png');
assert.deepEqual((await state()).names,[...original,'later.png']);assert.deepEqual((await state()).selected,['two.png']);
console.log('PASS: Ctrl+Z maps a reimported file to its current ID, avoids duplicates and preserves later additions.');

await click(2);await remove();await page.evaluate(()=>window.__qa.emitBatch({id:'undo-export-running',revision:1,status:'running',outputDirectory:'/synthetic/exports',items:[]}));
const guarded=(await calls('undo_photo_removal')).length;await page.focus(thumb(0));await page.keyboard.press('Meta+z');
assert.equal((await calls('undo_photo_removal')).length,guarded);assert.equal(await page.evaluate(()=>document.querySelector('.undo-photo-removal').disabled),true);
await page.evaluate(()=>window.__qa.emitBatch({id:'undo-export-running',revision:2,status:'completed',outputDirectory:'/synthetic/exports',items:[]}));await undo(7);
await page.click('#inspector-watermark');await page.focus('#font-search');await page.keyboard.press('Meta+z');assert.equal((await calls('undo_photo_removal')).length,guarded+1);
await page.click('#inspector-frame');await click(6);await remove();await page.selectOption('.language-select','zh-CN');
await page.cdp('Emulation.setDeviceMetricsOverride',{width:900,height:650,deviceScaleFactor:1,mobile:false});
assert.match(await page.evaluate(()=>document.querySelector('.undo-photo-removal').textContent),/撤销移除/);
assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
await page.screenshot({path:screenshotDirectory+'/undo-900x650.png'});
await page.click('.undo-photo-removal');await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===7);
console.log('PASS: active export blocks catalog undo, font controls retain native behavior, and the Chinese action fits a compact window.');
console.log('All removal undo fixture checks passed.');await task.finish({keep:[]});
