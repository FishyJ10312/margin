// Run with Vite on port 1420: ego-browser nodejs < tests/ui/drop-acceptance.mjs
// Native events are mocked here; desktop file-drop acceptance is recorded separately.
import assert from 'node:assert/strict';
const task=await taskSpace('Liubai batch drag acceptance');
console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html?empty&slow-start');
await page.waitForFunction(()=>window.__qa?.importControl.stateRequested);
await page.evaluate(()=>{window.__qa.drop(['/synthetic/one.jpg','/synthetic/two.png','/synthetic/three.heic']);window.__qa.releaseStartup();});
await page.waitForFunction(()=>window.__qa.dropReady&&window.__qa.importControl.stateReturned);
const startup=await page.evaluate(()=>window.__qa.requests.filter(v=>v.cmd==='import_photos').map(v=>v.args.paths));
console.log({startupImportRequests:startup});
assert.deepEqual(startup,[['/synthetic/one.jpg','/synthetic/two.png','/synthetic/three.heic']],'A group dropped while the workspace loads must not be lost');
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===3);
await page.evaluate(()=>{window.__qa.importControl.importDelay=120;window.__qa.drop(['/synthetic/four.jpg']);window.__qa.drop(['/synthetic/five.png','/synthetic/five.png']);});
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===5&&window.__qa.importControl.activeImports===0);
await page.evaluate(()=>window.__qa.drop(['/synthetic/corrupt.jpg','/synthetic/notes.txt','/synthetic/six.jpg']));
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===6&&window.__qa.importControl.activeImports===0);
const result=await page.evaluate(()=>({photos:document.querySelectorAll('.filmstrip .thumbnail').length,maxConcurrentImports:window.__qa.importControl.maxImports,calls:window.__qa.requests.filter(v=>v.cmd==='import_photos').map(v=>v.args.paths),feedback:document.querySelector('.import-feedback')?.textContent}));
assert.equal(result.maxConcurrentImports,1,'Repeated groups must queue instead of decoding concurrently');
assert.equal(result.photos,6,'Duplicates, text and one damaged image must not hide valid photos');
console.log(result);

await page.goto('http://127.0.0.1:1420/tests/ui/app.html?empty&fail-progress');
await page.waitForFunction(()=>window.__qa.dropReady&&window.__qa.importControl.stateReturned);
await page.evaluate(()=>window.__qa.drop(['/synthetic/still-imports.jpg']));
await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===1);
assert.equal(await page.evaluate(()=>document.body.textContent.includes('Fixture progress listener unavailable')),true);
console.log('PASS: a batch-progress listener failure does not disable file drops.');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html?empty&fail-drop');
await page.waitForFunction(()=>window.__qa.importControl.stateReturned&&document.querySelector('.empty-state h2'));
assert.equal(await page.evaluate(()=>document.body.textContent.includes('Fixture drop listener unavailable')),true);
console.log('PASS: unavailable native drops leave an actionable warning and usable file picker.');

await page.goto('http://127.0.0.1:1420/tests/ui/app.html?empty');
await page.waitForFunction(()=>window.__qa.dropReady&&window.__qa.importControl.stateReturned);
await page.evaluate(()=>window.__qa.dragEnter(Array.from({length:7},(_,i)=>'/synthetic/'+i+'.jpg')));
await page.waitForSelector('.drop-overlay');
assert.match(await page.evaluate(()=>document.querySelector('.drop-overlay strong').textContent),/7/);
await page.evaluate(()=>window.__qa.dragLeave());
await page.waitForSelector('.drop-overlay',{state:'hidden'});
console.log('PASS: hovering a seven-photo group shows the count; cancelling clears the overlay.');
