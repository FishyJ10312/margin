// Keyboard shortcut sheet opens from the header and with "?", not while typing; labelled model entry.
// Fixture only: saving a model configuration makes no model request.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin shortcut sheet');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
await page.selectOption('.language-select','en');
await page.click('.filmstrip .thumbnail >> nth=0');
await page.keyboard.press('Shift+Slash');
await page.waitForFunction(()=>document.querySelector('.shortcut-sheet')?.open);
const rows=await page.evaluate(()=>[...document.querySelectorAll('.shortcut-row dt')].map(d=>d.textContent));
assert.ok(rows.includes('Reorder')&&rows.includes('Undo')&&rows.includes('Fit to window'),rows.join(', '));
await page.keyboard.press('Escape');
await page.waitForFunction(()=>!document.querySelector('.shortcut-sheet').open);
console.log('PASS: ? opens the shortcut sheet and Escape closes it');
await page.click('.shortcuts-button');
await page.waitForFunction(()=>document.querySelector('.shortcut-sheet')?.open);
await page.keyboard.press('Escape');
await page.focus('#frame-hex');
await page.keyboard.press('Shift+Slash');
assert.equal(await page.evaluate(()=>document.querySelector('.shortcut-sheet').open),false,'Typing ? in a field does not open the sheet');
console.log('PASS: header button opens it; typing ? in a field does not');

// The model entry is labelled, and an advanced connection that is in use is not hidden.
assert.equal(await page.evaluate(()=>document.querySelector('.model-config-trigger').textContent.trim()),'Models');
await page.click('.model-config-trigger');
await page.waitForSelector('.models-config .other-connections');
assert.equal(await page.evaluate(()=>document.querySelector('.other-connections').open),false,'ChatGPT users see no advanced options');
await page.click('.other-connections > summary');
await page.evaluate(()=>[...document.querySelectorAll('.other-connections button')].find(b=>b.querySelector('strong').textContent==='Compatible vision API').click());
await page.fill('#configured-base-url','https://vision.example.test/v1');
await page.fill('#configured-api-model','fixture-vision');
await page.click('button[form="model-config-form"]');
await page.waitForFunction(()=>window.__qa.aiConfig.provider==='api');
await page.keyboard.press('Escape');
await page.waitForFunction(()=>!document.querySelector('.models-config'));
await page.click('.model-config-trigger');
await page.waitForSelector('#configured-base-url');
assert.equal(await page.evaluate(()=>document.querySelector('.other-connections').open),true,'The saved advanced connection stays visible');
console.log('PASS: model entry is labelled and keeps the active advanced connection visible');
await task.finish();
