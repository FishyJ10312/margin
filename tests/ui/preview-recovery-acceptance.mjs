// Real Svelte preview pump with controlled native completion; no live photos or accounts.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin reader review');console.log({spaceId:task.spaceId});const page=task.page('p1');
async function run(aba){
 await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
 await page.waitForFunction(()=>window.__qa?.previewControl.rendered.length>0&&document.querySelector('.zoom-photo')?.complete);
 console.log(await page.snapshot());
 await page.selectOption('.language-select','en');
 await page.evaluate(()=>window.__qa.drop(['/synthetic/another.png']));
 await page.waitForFunction(()=>document.querySelectorAll('.filmstrip .thumbnail').length===2&&window.__qa.importControl.activeImports===0);
 await page.evaluate(()=>{const q=window.__qa;q.previewControl.hold=true;q.observedPreviews=[];q.previewObserver=new MutationObserver(records=>{for(const r of records)if(r.target instanceof HTMLImageElement&&r.target.classList.contains('zoom-photo'))q.observedPreviews.push(r.target.src);});q.previewObserver.observe(document.body,{subtree:true,attributes:true,attributeFilter:['src']});});
 await page.fill('#frame-hex','#ff0405');
 await page.waitForFunction(()=>window.__qa.previewControl.pending.length===1);
 if(aba){await page.click('.filmstrip [aria-label="Preview another.png"]');await page.click('.filmstrip [aria-label="Preview synthetic-landscape.png"]');}
 await page.fill('#frame-hex','#0405ff');
 await page.evaluate(()=>window.__qa.previewControl.pending.shift()());
 await page.waitForFunction(()=>window.__qa.previewControl.pending.length===1&&window.__qa.previewControl.rendered.at(-1).settings.frameColor.toLowerCase()==='#ff0405');
 const held=await page.evaluate(()=>({src:document.querySelector('.zoom-photo').src,old:window.__qa.previewControl.rendered.at(-1).dataUrl}));
 await page.evaluate(()=>window.__qa.previewControl.pending.shift()());
 await page.waitForFunction(()=>window.__qa.previewControl.active===0&&window.__qa.previewControl.rendered.at(-1).settings.frameColor.toLowerCase()==='#0405ff'&&document.querySelector('.zoom-photo').src===window.__qa.previewControl.rendered.at(-1).dataUrl);
 const result=await page.evaluate(()=>{const q=window.__qa;q.previewObserver.disconnect();return {observed:q.observedPreviews,maxActive:q.previewControl.maxActive,last:q.previewControl.rendered.at(-1).settings.frameColor};});
 assert.notEqual(held.src,held.old,'An obsolete successful render must not replace the current view while the latest render is pending');
 assert.ok(!result.observed.includes(held.old),'An obsolete result must never flash, including A → B → A selection');assert.equal(result.maxActive,1);console.log('PASS:',aba?'A → B → A keeps the latest revision':'same photo keeps the latest settings',result.last);
}
await run(false);await run(true);
console.log('PASS: stale preview completions cannot overwrite the current requested view.');
