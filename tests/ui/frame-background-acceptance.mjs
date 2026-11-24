// Blurred-photo border fill: UI control, saved settings, per-photo scope, undo, overview.
// Synthetic catalog only; no model calls.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin blurred border fill');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/B.jpg','/synthetic/C.jpg']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===3&&!window.__qa.importControl.activeImports);

assert.equal(await page.evaluate(()=>window.__qa.savedSettings.background?.kind??'color'),'color','Solid fill is the default');
assert.equal(await page.evaluate(()=>!!document.querySelector('#background-blur')),false,'Blur sliders stay hidden for a solid fill');
await page.click('.background-kind button >> nth=1');
await page.waitForFunction(()=>window.__qa.savedSettings.background?.kind==='blur');
await page.waitForSelector('#background-blur');
await page.evaluate(()=>{const r=document.querySelector('#background-dim');r.value='0.4';r.dispatchEvent(new Event('input',{bubbles:true}));});
await page.waitForFunction(()=>Math.abs(window.__qa.savedSettings.background.dim-0.4)<1e-9);
const saved=await page.evaluate(()=>window.__qa.savedSettings.background);
assert.deepEqual(saved,{kind:'blur',blur:0.6,dim:0.4});
await page.waitForFunction(()=>window.__qa.previewControl.rendered.at(-1)?.settings.background?.dim===0.4);
console.log('PASS: blurred fill saves to shared settings and reaches the preview request');

// Per-photo edit leaves other photos on the shared fill.
await page.click('.filmstrip .thumbnail >> nth=1');
await page.click('.settings-scope button >> nth=1');
await page.click('.background-kind button >> nth=0');
await page.waitForFunction(()=>window.__qa.catalog.photos[1].settings?.background?.kind==='color');
assert.equal(await page.evaluate(()=>window.__qa.savedSettings.background.kind),'blur');
assert.equal(await page.evaluate(()=>window.__qa.catalog.photos[0].settings??null),null);
console.log('PASS: a per-photo solid fill does not change the shared blurred fill');

await page.click('.header-undo');
await page.waitForFunction(()=>window.__qa.catalog.photos[1].settings?.background?.kind!=='color');
console.log('PASS: undo restores the fill');

await page.click('.overview-toggle');
await page.waitForSelector('.overview-item');
assert.ok(await page.evaluate(()=>document.querySelectorAll('.overview-backdrop').length>=2),'Overview shows the blurred fill');
console.log('PASS: overview previews blurred fills');
await task.finish();
