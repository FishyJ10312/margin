// Real UI + synthetic catalog: overview, copy/paste settings, photo colours, layout.
// No model calls or user photos.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin workspace redesign');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/B.jpg','/synthetic/C.jpg']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===3&&!window.__qa.importControl.activeImports);
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const photos=()=>page.evaluate(()=>window.__qa.catalog.photos.map(p=>p.settings??null));

// Layout: thumbnails sit under the stage, the inspector keeps the full height.
const box=await page.evaluate(()=>{const r=s=>document.querySelector(s).getBoundingClientRect();return {strip:r('.filmstrip-section'),stage:r('.photo-viewport'),panel:r('.settings-panel'),header:r('.app-header'),zoom:r('.zoom-toolbar')};});
assert.ok(box.strip.y>=box.stage.bottom-1&&box.strip.right<=box.panel.x+1,'Filmstrip runs below the stage');
assert.ok(Math.abs(box.panel.y-box.header.bottom)<=1&&box.panel.bottom>=box.strip.bottom-1,'Inspector spans the full height');
assert.ok(box.zoom.y>box.stage.y&&box.zoom.bottom<=box.stage.bottom,'Zoom controls float inside the stage');
console.log('PASS: bottom filmstrip, full-height inspector and floating zoom controls');

// Photo colours set an exact frame colour on the shared settings.
await page.waitForSelector('.photo-swatch');
const suggested=await page.evaluate(()=>document.querySelector('.photo-swatch').title);
assert.match(suggested,/^#[0-9A-F]{6}$/);
await page.click('.photo-swatch >> nth=0');
await page.waitForFunction(c=>window.__qa.savedSettings.frameColor.toUpperCase()===c,suggested);
console.log('PASS: photo colour suggestion applies its exact HEX');

// Copy one photo's own look and paste it onto two others; undo restores them.
await page.click(thumb(0));
await page.click('.settings-scope button >> nth=1');
await page.click('.ratio-grid button >> nth=1');
await page.click('.palette-swatch[aria-label*="#171A17"]');
await page.waitForFunction(()=>window.__qa.catalog.photos[0].settings?.frameColor==='#171A17');
await page.click('.copy-settings');
await page.click(thumb(1));await page.keyboard.down('Shift');await page.click(thumb(2));await page.keyboard.up('Shift');
assert.match(await page.evaluate(()=>document.querySelector('.paste-settings').textContent),/Paste to 2/);
await page.click('.paste-settings');
await page.waitForFunction(()=>window.__qa.catalog.photos.slice(1).every(p=>p.settings?.frameColor==='#171A17'&&p.settings?.ratio?.width===1));
const pasted=await photos();
assert.deepEqual(pasted[1].watermark,pasted[0].watermark);
assert.equal(await page.evaluate(()=>document.querySelector('.settings-scope button.active').textContent.includes('Edit 2 selected')),true);
await page.click('.header-undo');
await page.waitForFunction(()=>window.__qa.catalog.photos.slice(1).every(p=>!p.settings));
assert.equal((await photos())[0].frameColor,'#171A17');
await page.click('.header-redo');
await page.waitForFunction(()=>window.__qa.catalog.photos.slice(1).every(p=>p.settings?.frameColor==='#171A17'));
console.log('PASS: copy/paste settings applies to the selection with undo and redo');

// Overview reflects each photo's ratio and selects like the filmstrip.
await page.click('.overview-toggle');
await page.waitForSelector('.overview-item');
assert.equal(await page.evaluate(()=>document.querySelectorAll('.overview-item').length),3);
const square=await page.evaluate(()=>{const r=document.querySelectorAll('.overview-canvas')[1].getBoundingClientRect();return Math.abs(r.width-r.height)<1;});
assert.equal(square,true,'A 1:1 photo shows a square canvas');
await page.click('.overview-item >> nth=0');
assert.deepEqual(await page.evaluate(()=>[...document.querySelectorAll('.overview-item.selected')].length),1);
await page.evaluate(()=>document.querySelectorAll('.overview-item')[2].dispatchEvent(new MouseEvent('dblclick',{bubbles:true})));
await page.waitForSelector('.photo-viewport');
assert.equal(await page.evaluate(()=>document.querySelector('.thumbnail.active').dataset.photoId===window.__qa.catalog.photos[2].id),true);
console.log('PASS: overview shows real ratios, selects, and opens a photo on double-click');
await task.finish();
