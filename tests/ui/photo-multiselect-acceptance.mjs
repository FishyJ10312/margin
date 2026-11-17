// Synthetic photos + mocked storage. Native original integrity is checked separately.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin multi selection');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());
await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png','/synthetic/four.png','/synthetic/five.png','/synthetic/six.png']));
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===6&&!window.__qa.importControl.activeImports);
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const selected=()=>page.evaluate(()=>[...document.querySelectorAll('.filmstrip .thumbnail')].flatMap((el,i)=>el.getAttribute('aria-pressed')==='true'?[i]:[]));
const removals=()=>page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd==='remove_photos'));
async function click(n,modifiers=[]){for(const key of modifiers)await page.keyboard.down(key);try{await page.click(thumb(n));}finally{for(const key of [...modifiers].reverse())await page.keyboard.up(key);}}
const dialog=()=>page.waitForSelector('.photo-removal-dialog[open]');
async function cancel(){await page.keyboard.press('Escape');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});}
await click(0);await click(2,['Meta']);await click(4,['Meta']);assert.deepEqual(await selected(),[0,2,4]);
assert.match(await page.evaluate(()=>document.querySelector('.selection-count').textContent),/3 selected/);
await click(2,['Meta']);assert.deepEqual(await selected(),[0,4]);await click(2,['Meta']);
await page.keyboard.press('Backspace');await dialog();
assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-title').textContent),/Remove 3 photos/);
assert.match(await page.evaluate(()=>document.querySelector('.removal-photo').textContent),/3 selected photos/);
assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-description').textContent),/Original files stay on your disk/);
assert.equal(await page.evaluate(()=>document.activeElement.textContent.trim()),'Cancel');
await cancel();assert.deepEqual(await selected(),[0,2,4]);assert.equal((await removals()).length,0);
console.log('PASS: Command toggles, visible count, batch Backspace confirmation, safe focus and cancel.');

await page.click(thumb(0),{button:'right'});await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');
assert.deepEqual(await selected(),[0,2,4]);assert.match(await page.evaluate(()=>document.querySelector('.photo-context-menu button').textContent),/Remove 3 photos/);
await page.click('.photo-context-menu button');await dialog();assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-title').textContent),/Remove 3 photos/);await cancel();
await page.focus(thumb(2));await page.keyboard.press('Shift+F10');await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');await page.keyboard.press('Escape');
assert.deepEqual(await selected(),[0,2,4]);
await page.click(thumb(1),{button:'right'});await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');assert.deepEqual(await selected(),[1]);await page.keyboard.press('Escape');
console.log('PASS: right-click inside selection preserves the group; outside selects only the clicked photo.');

await click(1);await click(4,['Shift']);assert.deepEqual(await selected(),[1,2,3,4]);
await click(2,['Shift']);assert.deepEqual(await selected(),[1,2]);await click(0,['Shift']);assert.deepEqual(await selected(),[0,1]);
await click(0);await click(3,['Meta']);await click(5,['Meta','Shift']);assert.deepEqual(await selected(),[0,3,4,5]);
await click(2);await click(2,['Meta']);assert.deepEqual(await selected(),[]);await page.keyboard.press('Backspace');
assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);assert.equal(await page.evaluate(()=>document.querySelector('.remove-photo').disabled),true);
console.log('PASS: fixed-anchor Shift grows, shrinks and reverses ranges; Command+Shift unions; empty selection cannot remove.');

await click(0);await click(2,['Meta']);await click(4,['Meta']);
await page.click('#inspector-watermark');await page.fill('#watermark-text','SignatureX');await page.keyboard.press('Backspace');
assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);assert.deepEqual(await selected(),[0,2,4]);
await page.focus('#font-search');await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.click('#inspector-frame');await page.click('.quick-templates-heading button');await page.waitForSelector('.templates-dialog[open]');await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);await page.keyboard.press('Escape');
assert.equal((await removals()).length,0);console.log('PASS: editing and other dialogs preserve selection and catalog.');

const expectedIds=await page.evaluate(()=>[...document.querySelectorAll('.thumbnail.selected')].map(el=>el.dataset.photoId));
await page.click('.remove-photo');await dialog();await page.evaluate(()=>window.__qa.removalControl.fail=true);await page.click('.confirm-photo-removal');
await page.waitForFunction(()=>document.querySelector('.removal-status').textContent.includes('Fixture cannot save catalog'));
assert.deepEqual(await selected(),[0,2,4]);assert.equal(await page.evaluate(()=>document.querySelectorAll('.filmstrip .thumbnail').length),6);
await page.evaluate(()=>{window.__qa.removalControl.fail=false;window.__qa.removalControl.hold=true;});await page.click('.confirm-photo-removal');
await page.waitForFunction(()=>window.__qa.removalControl.pending.length===1);await page.keyboard.press('Enter');assert.equal((await removals()).length,2);
await page.evaluate(()=>{window.__qa.removalControl.hold=false;window.__qa.removalControl.pending.shift()();});
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===3);
assert.deepEqual((await removals()).at(-1).args.ids,expectedIds);assert.deepEqual(await selected(),[2]);
assert.equal(await page.evaluate(()=>document.activeElement===document.querySelector('.thumbnail.active')),true);
console.log('PASS: failed batch save preserves photos/selection; retry sends exact IDs once and focuses a survivor.');

await click(0);await click(2,['Meta']);await page.click(thumb(0),{button:'right'});await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');
await page.click('.photo-context-menu button');await dialog();await page.click('.confirm-photo-removal');await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===1);
assert.equal((await removals()).at(-1).args.ids.length,2);
await page.click('.clear-photos');await dialog();await page.click('.confirm-photo-removal');await page.waitForFunction(()=>!!document.querySelector('.empty-state'));
await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.evaluate(()=>window.__qa.drop(['/synthetic/new-one.png','/synthetic/new-two.png']));await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===2);
assert.deepEqual(await selected(),[0]);await click(1,['Shift']);await page.selectOption('.language-select','zh-CN');await page.click('.remove-photo');await dialog();
assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-title').textContent),/2 张照片/);await cancel();
await page.evaluate(()=>window.__qa.emitBatch({id:'fixture-multi-export',revision:1,status:'running',items:[],outputDirectory:'/synthetic/exports'}));
await page.focus(thumb(0));await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);assert.equal(await page.evaluate(()=>document.querySelector('.remove-photo').disabled),true);
console.log('PASS: context-menu batch removal, empty state, re-import selection, Chinese count and export guard.');
console.log('All multi-selection fixture checks passed.');
await task.finish({keep:[]});
