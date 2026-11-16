// Run with ego-browser nodejs. Uses the real UI and isolated native replies.
import assert from 'node:assert/strict';
const task=await taskSpace('Liubai watermark workspace');
const page=task.page('p1');
console.log({spaceId:task.spaceId});
const dock='.content-area > .watermark-detail';
async function viewport(width,height){await page.cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});}
async function settled(){await page.waitForFunction(()=>document.querySelector('.watermark-detail')?.getAttribute('aria-busy')==='false'&&document.querySelector('.watermark-detail img')?.complete);}
async function prepare(query=''){
 await page.goto('http://127.0.0.1:1420/tests/ui/app.html'+query);
 await page.waitForSelector(dock+' img');await page.selectOption('.language-select','en');
 await settled();await page.click('.watermark-section details.advanced-watermark > summary');
 await page.waitForSelector('.font-picker [role=option]');
}
async function geometry(){return page.evaluate(()=>{
 const box=selector=>{const r=document.querySelector(selector).getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:r.width,height:r.height};};
 return {dock:box('.watermark-detail'),photo:box('.photo-viewport'),panel:box('.settings-panel'),strip:box('.filmstrip-section'),list:box('.font-picker'),footer:box('.app-footer'),width:innerWidth,height:innerHeight,documentWidth:document.documentElement.scrollWidth};
});}
function visibleTogether(g){
 assert.ok(g.dock.x>=0&&g.dock.right<=g.panel.x&&g.dock.bottom<=g.strip.y,'Dock stays in the left workspace above the photo strip');
 assert.ok(g.photo.height>=100&&g.photo.bottom<=g.dock.y,'The full-photo viewer remains usable beside the persistent detail');
 assert.ok(g.list.y>=0&&g.list.bottom<=g.footer.y,'Keyboard font choices are visible');
 assert.ok(g.strip.bottom<=g.footer.y+1&&g.footer.bottom<=g.height+1,'Thumbnails and export controls fit the window');
 assert.equal(g.documentWidth,g.width,'No horizontal page overflow');
}
await viewport(1280,900);await prepare();
assert.equal(await page.evaluate(()=>document.querySelectorAll('.settings-panel .watermark-detail').length),0);
assert.deepEqual(await page.evaluate(()=>window.__qa.windowControl.inner),{width:2560,height:1800});
await page.focus('.font-picker');
const initial=await page.evaluate(()=>({src:document.querySelector('.watermark-detail img').src,panel:document.querySelector('.settings-panel').scrollTop}));
await page.press('.font-picker','ArrowDown');await settled();
const after=await page.evaluate(()=>({src:document.querySelector('.watermark-detail img').src,chosen:document.querySelector('.font-picker [aria-selected=true]').textContent,font:document.querySelector('.detail-font').textContent,focused:document.activeElement.classList.contains('font-picker'),panel:document.querySelector('.settings-panel').scrollTop}));
assert.ok(after.chosen.includes('Serif'));assert.equal(after.font,'站酷小薇体');assert.notEqual(after.src,initial.src,'The actual context image updates when changing font');assert.equal(after.focused,true);assert.equal(after.panel,initial.panel);
visibleTogether(await geometry());
await page.press('.font-picker','ArrowDown');await settled();
assert.equal(await page.evaluate(()=>document.querySelector('.detail-font').textContent),'Ma Shan Zheng');
console.log('PASS: keyboard font changes update the left context image without stealing focus or scrolling it away.');

// Hold a real request in flight, then choose a newer font repeatedly.
await page.evaluate(()=>{window.__qa.detailControl.delay=180;window.__qa.observed=[];const img=document.querySelector('.watermark-detail img');window.__qa.observer=new MutationObserver(()=>window.__qa.observed.push(img.src));window.__qa.observer.observe(img,{attributes:true,attributeFilter:['src']});});
await page.press('.font-picker','ArrowUp');await page.waitForFunction(()=>window.__qa.detailControl.active===1);
await page.press('.font-picker','ArrowDown');await page.press('.font-picker','ArrowDown');await settled();
const rapid=await page.evaluate(()=>{const rendered=window.__qa.detailControl.rendered;return {active:window.__qa.detailControl.maxActive,font:rendered.at(-1).settings.watermark.fontFamily,src:document.querySelector('.watermark-detail img').src,expected:rendered.at(-1).dataUrl,stale:rendered.at(-2).dataUrl,observed:window.__qa.observed,focus:document.activeElement.classList.contains('font-picker'),resizes:window.__qa.requests.filter(r=>r.cmd==='plugin:window|set_size').length};});
assert.equal(rapid.active,1);assert.equal(rapid.font,'Times New Roman');assert.equal(rapid.src,rapid.expected);assert.ok(!rapid.observed.includes(rapid.stale),'An obsolete in-flight font must never flash in the close-up');assert.equal(rapid.focus,true);assert.equal(rapid.resizes,1,'Font changes must not keep resizing the window');
await page.evaluate(()=>{window.__qa.observer.disconnect();window.__qa.detailControl.delay=0;});
console.log('PASS: rapid keyboard changes stay serial and only show the latest requested font.');

const beforeSize=await page.evaluate(()=>document.querySelector('.watermark-detail img').src);
await page.fill('input[aria-label="Watermark size percentage"]','4');await settled();
assert.notEqual(await page.evaluate(()=>document.querySelector('.watermark-detail img').src),beforeSize);
await page.fill('input[aria-label="Watermark horizontal offset"]','-40');await page.fill('input[aria-label="Watermark vertical offset"]','20');await settled();
assert.ok(await page.evaluate(()=>document.querySelector('.detail-position').textContent.includes('X 410 · Y 740')));
await page.selectOption('.watermark-detail select[aria-label="Watermark close-up zoom"]','2');
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail .fit-image').classList.contains('zoomed')),true);
await page.selectOption('.watermark-detail select[aria-label="Watermark close-up view"]','placement');
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail .fit-image').classList.contains('zoomed')),false);
console.log('PASS: size, offsets, zoom and whole-image placement stay synchronized in the embedded preview.');

for(const [width,height]of [[1180,800],[900,650]]){
 await viewport(width,height);await page.focus('.font-picker');await page.press('.font-picker','Home');await settled();
 visibleTogether(await geometry());
 console.log('PASS: photo, watermark, font list and export controls fit',width+'×'+height);
}
const withDock=(await geometry()).photo.height;
await page.click('.watermark-section .segmented button:first-child');await page.waitForSelector(dock,{state:'hidden'});
assert.ok((await geometryWithoutDock())>withDock+100,'Turning the watermark off returns space to the photo');
async function geometryWithoutDock(){return page.evaluate(()=>document.querySelector('.photo-viewport').getBoundingClientRect().height);}
await page.click('.watermark-section .segmented button:nth-child(2)');await page.waitForSelector(dock+' img');await settled();

// Exercise Retina scaling and a secondary monitor with a negative desktop origin.
const resized=await page.evaluate(async()=>{
 const c=window.__qa.windowControl;Object.assign(c,{scale:1.5,inner:{width:1350,height:975},outer:{width:1374,height:1020},position:{x:-1000,y:200},area:{position:{x:-1800,y:75},size:{width:1800,height:1155}}});
 await (await import('/src/lib/window-layout.ts')).ensureWatermarkWorkspace();
 return {inner:c.inner,outer:c.outer,position:c.position,area:c.area};
});
assert.deepEqual(resized.inner,{width:1752,height:1086});
assert.ok(resized.position.x>=resized.area.position.x+12&&resized.position.y>=resized.area.position.y+12);
assert.ok(resized.position.x+resized.outer.width<=resized.area.position.x+resized.area.size.width-12);
assert.ok(resized.position.y+resized.outer.height<=resized.area.position.y+resized.area.size.height-12);
for(const flag of ['fullscreen','maximized','rejectResize']){
 const unchanged=await page.evaluate(async flag=>{const c=window.__qa.windowControl;Object.assign(c,{inner:{width:900,height:650},outer:{width:900,height:678},scale:1,fullscreen:false,maximized:false,rejectResize:false});c[flag]=true;await (await import('/src/lib/window-layout.ts')).ensureWatermarkWorkspace();return c.inner;},flag);
 assert.deepEqual(unchanged,{width:900,height:650});
}
console.log('PASS: automatic growth respects display scale, work area and full-screen/maximized windows; declined resizing is harmless.');

await viewport(1280,900);await prepare('?long-font-list');await page.focus('.font-picker');await page.press('.font-picker','End');await settled();
const boundary=await page.evaluate(()=>{const list=document.querySelector('.font-picker');list.scrollTop=list.scrollHeight;const r=list.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,panel:document.querySelector('.settings-panel').scrollTop};});
await page.mouse.move(boundary.x,boundary.y,{label:'Scrolling within font options'});await page.mouse.wheel(0,380);
assert.equal(await page.evaluate(()=>document.querySelector('.settings-panel').scrollTop),boundary.panel);
visibleTogether(await geometry());
await page.selectOption('.language-select','zh-CN');
assert.ok(await page.evaluate(()=>document.querySelector('.detail-heading').textContent.includes('实际水印效果')));
console.log({screenshot:await page.screenshot({path:'/tmp/liubai-watermark-workspace-verified.png'})});
console.log('All watermark workspace checks passed. Photo/font/render replies were isolated fixtures; no model calls.');
