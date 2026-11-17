// Synthetic photos, mocked directory chooser and batch worker. No real account calls.
import assert from 'node:assert/strict';
const screenshotDirectory=(await import('node:path')).join((await import('node:os')).tmpdir(),'margin-ui-acceptance');
await (await import('node:fs/promises')).mkdir(screenshotDirectory,{recursive:true});
const task=await taskSpace('Margin selection export');console.log({spaceId:task.spaceId});
const page=task.page('p1');
await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png','/synthetic/four.png']));
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===4&&!window.__qa.importControl.activeImports);
const thumb=n=>`.filmstrip .thumbnail >> nth=${n}`;
const requests=()=>page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd==='start_batch'));
const ids=()=>page.evaluate(()=>[...document.querySelectorAll('.thumbnail.selected')].map(el=>el.dataset.photoId));
const countText=()=>page.evaluate(()=>document.querySelector('.start-export').textContent.trim());
async function add(n){await page.keyboard.down('Meta');try{await page.click(thumb(n));}finally{await page.keyboard.up('Meta');}}
async function open(){await page.click('.export-button');await page.waitForSelector('.export-review[open]');}
async function complete(){await page.evaluate(()=>window.__qa.emitBatch({id:'fixture-completed-'+Date.now(),revision:2,status:'completed',items:[],outputDirectory:'/synthetic/exports'}));await page.waitForSelector('.export-button');}
await page.click(thumb(0));await add(2);const selected=await ids();
assert.match(await page.evaluate(()=>document.querySelector('.export-button').textContent),/Export 2 selected/);
await open();assert.equal(await page.evaluate(()=>document.querySelector('input[name="export-scope"][value="selected"]').checked),true);
assert.match(await countText(),/Export 2 photos/);await page.keyboard.press('Backspace');
assert.equal(await page.evaluate(()=>document.querySelector('.photo-removal-dialog').open),false);
await page.click('input[name="export-scope"][value="all"]');assert.match(await countText(),/Export 4 photos/);
await page.focus('input[name="export-scope"][value="all"]');await page.keyboard.press('ArrowUp');
assert.equal(await page.evaluate(()=>document.querySelector('input[name="export-scope"][value="selected"]').checked),true);
await page.keyboard.press('Escape');assert.equal((await requests()).length,0);assert.deepEqual(await ids(),selected);
console.log('PASS: selected count, native keyboard range control, modal protection and cancel without dispatch.');

await open();await page.evaluate(()=>{window.__qa.exportControl.holdDirectory=true;window.__qa.exportControl.directory='/synthetic/selected';});
await page.click('.start-export');await page.waitForFunction(()=>window.__qa.exportControl.directoryPending.length===1);
await page.keyboard.press('Escape');assert.equal(await page.evaluate(()=>document.querySelector('.export-review').open),true);
await page.evaluate(()=>{document.querySelector('.start-export').click();window.__qa.drop(['/synthetic/added-while-choosing.png']);});
await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===5&&!window.__qa.importControl.activeImports);
await page.evaluate(()=>{window.__qa.exportControl.holdDirectory=false;window.__qa.exportControl.directoryPending.shift()();});
await page.waitForFunction(()=>!document.querySelector('.export-review').open);
assert.equal((await requests()).length,1);assert.deepEqual((await requests())[0].args.photoIds,selected);
await page.click(thumb(4));assert.deepEqual((await requests())[0].args.photoIds,selected);
await complete();console.log('PASS: selected IDs freeze before the directory chooser, no double start and later preview changes do not affect the batch.');

await open();await page.click('input[name="export-scope"][value="all"]');assert.match(await countText(),/Export 5 photos/);
await page.evaluate(()=>window.__qa.exportControl.directory='/synthetic/all');await page.click('#export-directory');
await page.waitForFunction(()=>document.querySelector('#export-directory').textContent.includes('/synthetic/all'));
const all=await page.evaluate(()=>[...document.querySelectorAll('.thumbnail')].map(el=>el.dataset.photoId));
await page.click('.start-export');await page.waitForFunction(()=>!document.querySelector('.export-review').open);
assert.deepEqual((await requests()).at(-1).args.photoIds,all);assert.equal((await requests()).at(-1).args.outputDirectory,'/synthetic/all');
await complete();console.log('PASS: explicit all-photo range sends the complete current catalog and chosen directory.');

await open();assert.match(await countText(),/Export 1 photo/);
await page.evaluate(()=>window.__qa.exportControl.fail=true);await page.click('.start-export');
await page.waitForSelector('.export-error');assert.match(await page.evaluate(()=>document.querySelector('.export-error').textContent),/Fixture export cannot start/);
await page.evaluate(()=>{window.__qa.exportControl.fail=false;window.__qa.exportControl.holdStart=true;});await page.click('.start-export');
await page.waitForFunction(()=>window.__qa.exportControl.startPending.length===1);const inFlight=(await requests()).length;
await page.keyboard.press('Escape');await page.evaluate(()=>document.querySelector('.start-export').click());assert.equal((await requests()).length,inFlight);
await page.evaluate(()=>{window.__qa.exportControl.holdStart=false;window.__qa.exportControl.startPending.shift()();});await page.waitForFunction(()=>!document.querySelector('.export-review').open);await complete();
console.log('PASS: start failure stays visible and retry does not allow duplicate dispatch.');

await page.goto('http://127.0.0.1:1420/tests/ui/app.html');await page.waitForFunction(()=>window.__qa?.dropReady&&document.querySelector('.zoom-photo')?.complete);
await page.selectOption('.language-select','zh-CN');await page.evaluate(()=>window.__qa.drop(['/synthetic/two.png','/synthetic/three.png']));await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===3&&!window.__qa.importControl.activeImports);
await page.click(thumb(0));await add(2);await open();assert.match(await countText(),/开始导出 2 张/);
await page.evaluate(()=>window.__qa.exportControl.directory=null);await page.click('.start-export');
await page.waitForFunction(()=>!document.querySelector('.start-export').disabled);assert.equal((await requests()).length,0);
await page.cdp('Emulation.setDeviceMetricsOverride',{width:900,height:650,deviceScaleFactor:1,mobile:false});
const box=await page.evaluate(()=>{const d=document.querySelector('.export-review').getBoundingClientRect(),f=document.querySelector('.export-review > footer').getBoundingClientRect();return {left:d.left,right:d.right,bottom:d.bottom,footerBottom:f.bottom,width:innerWidth,height:innerHeight,documentWidth:document.documentElement.scrollWidth};});
assert.ok(box.left>=0&&box.right<=box.width&&box.bottom<=box.height&&box.footerBottom<=box.height);assert.ok(box.documentWidth<=box.width);
await page.screenshot({path:screenshotDirectory+'/export-900x650.png'});
await page.keyboard.press('Escape');await page.click(thumb(0));await add(0);
assert.match(await page.evaluate(()=>document.querySelector('.export-button').textContent),/导出选中 0 张/);
assert.equal(await page.evaluate(()=>document.querySelector('.export-button').disabled),true);
await page.click('.export-options');await page.waitForSelector('.export-review[open]');
assert.equal(await page.evaluate(()=>document.querySelector('input[name="export-scope"][value="selected"]').checked),true);
assert.equal(await page.evaluate(()=>document.querySelector('input[name="export-scope"][value="selected"]').disabled),true);
assert.equal(await page.evaluate(()=>document.querySelector('.start-export').disabled),true);
await page.click('input[name="export-scope"][value="all"]');assert.match(await countText(),/开始导出 3 张/);
await page.keyboard.press('Escape');console.log('PASS: Chinese counts, canceled chooser sends nothing, compact dialog keeps footer visible, and all-photo export requires an explicit range when nothing is selected.');
console.log('All selected-export fixture checks passed.');await task.finish({keep:[]});
