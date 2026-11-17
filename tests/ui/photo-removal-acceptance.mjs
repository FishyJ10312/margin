// Real UI and app context menu with synthetic photos and mocked storage.
// Original-file integrity is checked separately on Mac.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin photo removal');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/second.png','/synthetic/third.png']));
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===3&&!window.__qa.importControl.activeImports);
const count=()=>page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd==='remove_photos').length);

await page.click('.remove-photo');await page.waitForSelector('.photo-removal-dialog[open]');
assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-description').textContent),/Original files stay on your disk/);
assert.equal(await page.evaluate(()=>document.activeElement.textContent.trim()),'Cancel');
assert.equal(await count(),0,'Opening confirmation must not remove anything');
await page.keyboard.press('Escape');await page.waitForSelector('.photo-removal-dialog[open]',{state:'hidden'});
assert.equal(await count(),0);assert.equal(await page.evaluate(()=>document.activeElement.classList.contains('remove-photo')),true);
const target=await page.evaluate(()=>{const b=document.querySelector('.remove-photo').getBoundingClientRect();return {width:b.width,height:b.height,text:document.querySelector('.remove-photo').textContent};});assert.ok(target.width>75&&target.height>=32);assert.match(target.text,/Remove photo/);
console.log('PASS: visible text button, safe initial focus, cancel/Escape and explicit original-file wording.');

await page.click('.filmstrip .thumbnail >> nth=0');await page.keyboard.press('Backspace');await page.waitForSelector('.photo-removal-dialog[open]');
assert.match(await page.evaluate(()=>document.querySelector('.removal-photo').textContent),/synthetic-landscape.png/);
await page.click('.confirm-photo-removal');await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===2);
assert.equal(await count(),1);assert.match(await page.evaluate(()=>document.querySelector('.thumbnail.selected').getAttribute('aria-label')),/second.png/);
assert.equal(await page.evaluate(()=>document.activeElement===document.querySelector('.thumbnail.selected')),true);
console.log('PASS: Backspace confirms exactly one removal and selects/focuses the next photo.');

await page.click('.filmstrip .thumbnail >> nth=1',{button:'right'});await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');
assert.equal(await count(),1);assert.match(await page.evaluate(()=>document.querySelector('.thumbnail.selected').getAttribute('aria-label')),/third.png/);
await page.click('.photo-context-menu button');await page.waitForSelector('.photo-removal-dialog[open]');assert.match(await page.evaluate(()=>document.querySelector('.removal-photo').textContent),/third.png/);
await page.keyboard.press('Escape');
await page.focus('.filmstrip .thumbnail >> nth=1');await page.keyboard.press('Shift+F10');await page.waitForFunction(()=>document.querySelector('.photo-context-menu').dataset.open==='true');await page.click('.photo-context-menu button');await page.waitForSelector('.photo-removal-dialog[open]');await page.keyboard.press('Escape');
assert.equal(await page.evaluate(()=>document.querySelectorAll('.photo-context-menu').length),1,'Repeated menu use must reuse one menu');
console.log('PASS: right-click and keyboard context menu target the clicked photo; one menu is reused.');

await page.click('#inspector-watermark');await page.fill('#watermark-text','Keep editing');await page.keyboard.press('Backspace');
assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.focus('#font-search');await page.keyboard.press('Backspace');
assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.click('#inspector-frame');await page.focus('#frame-hex');await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);await page.fill('#frame-hex','#FFFFFF');
await page.click('.quick-templates-heading button');await page.waitForSelector('.templates-dialog[open]');await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);await page.keyboard.press('Escape');
assert.equal(await count(),1);console.log('PASS: text, font search, settings and other modal dialogs do not trigger photo removal.');

await page.click('.filmstrip .thumbnail >> nth=0');await page.keyboard.press('Delete');await page.waitForSelector('.photo-removal-dialog[open]');await page.evaluate(()=>window.__qa.removalControl.fail=true);await page.click('.confirm-photo-removal');await page.waitForFunction(()=>document.querySelector('.removal-status').textContent.includes('Fixture cannot save catalog'));
assert.equal(await page.evaluate(()=>document.querySelectorAll('.filmstrip .thumbnail').length),2);
await page.evaluate(()=>{window.__qa.removalControl.fail=false;window.__qa.removalControl.hold=true;});await page.click('.confirm-photo-removal');await page.waitForFunction(()=>window.__qa.removalControl.pending.length===1);await page.keyboard.press('Enter');assert.equal(await count(),3);await page.evaluate(()=>{window.__qa.removalControl.hold=false;window.__qa.removalControl.pending.shift()();});await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===1);
console.log('PASS: a failed save preserves the photo; an in-flight confirmation cannot send duplicate removals.');

await page.selectOption('.language-select','zh-CN');await page.click('.clear-photos');await page.waitForSelector('.photo-removal-dialog[open]');assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-description').textContent),/原始文件仍保留/);await page.click('.confirm-photo-removal');await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===0&&!!document.querySelector('.empty-state'));
await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.evaluate(()=>window.__qa.drop(['/synthetic/one.png','/synthetic/two.png']));await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===2);await page.click('.clear-photos');await page.waitForSelector('.photo-removal-dialog[open]');assert.match(await page.evaluate(()=>document.querySelector('#photo-removal-title').textContent),/2/);await page.keyboard.press('Escape');
await page.evaluate(()=>window.__qa.emitBatch({id:'fixture-running',revision:1,status:'running',items:[],outputDirectory:'/synthetic/exports'}));await page.focus('.filmstrip .thumbnail >> nth=0');await page.keyboard.press('Backspace');assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);assert.equal(await page.evaluate(()=>document.querySelector('.remove-photo').disabled),true);
console.log('PASS: Chinese/English confirmation, last-photo empty state, clear-all confirmation and active-export protection.');
console.log('All photo-removal fixture checks passed.');
