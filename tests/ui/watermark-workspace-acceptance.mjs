// Run with ego-browser nodejs. Uses the real UI and isolated native replies.
import assert from 'node:assert/strict';
const task=await taskSpace('Liubai corner watermark preview');
const page=task.page('p1');
console.log({spaceId:task.spaceId});
const dock='.photo-viewport > .watermark-detail';
async function viewport(width,height){await page.cdp('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});}
async function settled(){await page.waitForFunction(()=>document.querySelector('.watermark-detail')?.getAttribute('aria-busy')==='false'&&document.querySelector('.watermark-detail img')?.complete);}
async function prepare(query=''){
 await page.goto('http://127.0.0.1:1420/tests/ui/app.html'+query);
 await page.waitForSelector(dock);
 if(await page.evaluate(()=>!document.querySelector('.watermark-detail').open))await page.click('.watermark-detail > summary');
 await page.waitForSelector(dock+' img');await page.selectOption('.language-select','en');
 await settled();await page.click('#inspector-watermark');
 await page.waitForSelector('.font-picker [data-index="0"]');
}
async function geometry(){return page.evaluate(()=>{
 const box=selector=>{const r=document.querySelector(selector).getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:r.width,height:r.height};};
 return {dock:box('.watermark-detail'),photo:box('.photo-viewport'),panel:box('.settings-panel'),strip:box('.filmstrip-section'),list:box('.font-picker'),footer:box('.library-footer'),width:innerWidth,height:innerHeight,documentWidth:document.documentElement.scrollWidth};
});}
function visibleTogether(g){
 assert.ok(Math.abs(g.dock.right-(g.photo.right-8))<=1&&Math.abs(g.dock.y-(g.photo.y+8))<=1,'Close-up stays in the top-right photo corner');
 assert.ok(g.dock.width<=160&&g.dock.height<=140,'Close-up remains a miniature overlay');
 assert.ok(Math.abs(g.photo.right-g.panel.x)<=1,'The main photo uses all available width instead of reserving a side column');
 assert.ok(g.photo.height>=300,'Main photo keeps its full height, including at the smallest desktop size');
 assert.ok(g.list.y>=0&&g.list.bottom<=g.height&&(g.list.bottom<=g.footer.y||g.list.x>=g.footer.right),'Keyboard font choices are visible');
 assert.ok(g.strip.y>=g.footer.y-1&&g.strip.bottom<=g.footer.bottom+1&&g.footer.bottom<=g.height+1,'Thumbnails and export controls fit the window');
 assert.equal(g.documentWidth,g.width,'No horizontal page overflow');
}
await viewport(1280,900);await prepare();
assert.equal(await page.evaluate(()=>document.querySelectorAll('.settings-panel .watermark-detail').length),0);
assert.deepEqual(await page.evaluate(()=>window.__qa.windowControl.inner),{width:2360,height:1600});
await page.focus('.font-picker');
const initial=await page.evaluate(()=>({src:document.querySelector('.watermark-detail img').src,panel:document.querySelector('#watermark-panel').scrollTop}));
await page.press('.font-picker','ArrowDown');await settled();
const after=await page.evaluate(()=>({src:document.querySelector('.watermark-detail img').src,chosen:document.querySelector('.font-picker [aria-selected=true]').textContent,font:document.querySelector('.detail-font').textContent,focused:document.activeElement.classList.contains('font-picker'),panel:document.querySelector('#watermark-panel').scrollTop}));
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
assert.equal(rapid.active,1);assert.equal(rapid.font,'Times New Roman');assert.equal(rapid.src,rapid.expected);assert.ok(!rapid.observed.includes(rapid.stale),'An obsolete in-flight font must never flash in the close-up');assert.equal(rapid.focus,true);assert.equal(rapid.resizes,0,'The compact gallery must not resize the window when changing fonts');
await page.evaluate(()=>{window.__qa.observer.disconnect();window.__qa.detailControl.delay=0;});
console.log('PASS: rapid keyboard changes stay serial and only show the latest requested font.');

const beforeSize=await page.evaluate(()=>document.querySelector('.watermark-detail img').src);
await page.fill('input[aria-label="Watermark size percentage"]','4');await settled();
assert.notEqual(await page.evaluate(()=>document.querySelector('.watermark-detail img').src),beforeSize);
await page.click('.position-details > summary');
// The expanded controls can sit below the inspector's own scroll viewport.
// Bring them into view just as a user scrolling the inspector would.
await page.evaluate(()=>document.querySelector('input[aria-label="Watermark horizontal offset"]').scrollIntoView({block:'center',behavior:'instant'}));
await page.fill('input[aria-label="Watermark horizontal offset"]','-40');await page.fill('input[aria-label="Watermark vertical offset"]','20');await settled();
assert.ok(await page.evaluate(()=>document.querySelector('.detail-image').title.includes('X 410 · Y 740')));
await page.selectOption('.watermark-detail select[aria-label="Watermark close-up zoom"]','2');
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail .fit-image').classList.contains('zoomed')),true);
const wheelTarget=await page.evaluate(()=>{const r=document.querySelector('.detail-image').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,zoom:document.querySelector('.zoom-toolbar input[type=number]').value,transform:document.querySelector('.zoom-photo').style.transform};});
await page.mouse.move(wheelTarget.x,wheelTarget.y,{label:'Inspecting the watermark overlay'});await page.mouse.wheel(0,100);
assert.deepEqual(await page.evaluate(()=>({zoom:document.querySelector('.zoom-toolbar input[type=number]').value,transform:document.querySelector('.zoom-photo').style.transform})),{zoom:wheelTarget.zoom,transform:wheelTarget.transform},'Scrolling the overlay must not zoom or pan the main photo');
const canvasPoint=await page.evaluate(()=>{const r=document.querySelector('.photo-viewport').getBoundingClientRect();return {x:r.x+30,y:r.bottom-40};});
await page.mouse.move(canvasPoint.x,canvasPoint.y,{label:'Zooming the main photo'});await page.mouse.wheel(0,100);
assert.notEqual(await page.evaluate(()=>document.querySelector('.zoom-toolbar input[type=number]').value),wheelTarget.zoom,'Photo wheel zoom remains available outside the overlay');
await page.click('.zoom-toolbar [aria-pressed]');
await page.selectOption('.watermark-detail select[aria-label="Watermark close-up view"]','placement');
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail .fit-image').classList.contains('zoomed')),false);
console.log('PASS: size/offsets/zoom stay synchronized; overlay wheel input is isolated from main-photo zoom.');

for(const [width,height]of [[1180,800],[900,650]]){
 await viewport(width,height);await page.evaluate(()=>document.querySelector('.font-picker').scrollIntoView({block:'center',behavior:'instant'}));await page.focus('.font-picker');await page.press('.font-picker','Home');await settled();
 const sized=await geometry();assert.equal(sized.width,width);assert.equal(sized.height,height);visibleTogether(sized);
 console.log('PASS: photo, watermark, font list and export controls fit',width+'×'+height);
}
const expanded=await geometry();
const settingsBeforeCollapse=await page.evaluate(()=>JSON.stringify(window.__qa.requests.filter(r=>r.cmd==='get_preview').at(-1).args.settings));
await page.click('.watermark-detail > summary');
await page.waitForFunction(()=>!document.querySelector('.watermark-detail').open&&document.querySelector('.watermark-detail').classList.contains('collapsed'));
const folded=await geometry();
assert.ok(folded.dock.width<=28,'Collapsed overlay becomes a single magnifier');
assert.equal(folded.photo.width,expanded.photo.width,'Folding never changes the full-width photo layout');
assert.equal(folded.photo.height,expanded.photo.height,'Folding never changes the full-height photo layout');
assert.equal(await page.evaluate(()=>document.activeElement===document.querySelector('.watermark-detail > summary')),true);
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail img')),null,'Collapsed close-up has no mounted image surface');
assert.equal(await page.evaluate(()=>JSON.stringify(window.__qa.requests.filter(r=>r.cmd==='get_preview').at(-1).args.settings)),settingsBeforeCollapse,'Folding the preview must not disable or change the exported watermark');
await page.reload();await page.waitForSelector(dock);await page.waitForFunction(()=>document.querySelector('.watermark-detail').getAttribute('aria-busy')==='false');
assert.equal(await page.evaluate(()=>document.querySelector('.watermark-detail').open),false,'Collapsed preference survives reopening');
await page.click('#inspector-watermark');await page.waitForSelector('.font-picker [data-index="0"]');
assert.equal(await page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd.startsWith('plugin:window|')).length),0,'Collapsed preview must not enlarge the window when loading photos or fonts');
await page.press('.watermark-detail > summary','Enter');await settled();
assert.equal(await page.evaluate(()=>document.activeElement===document.querySelector('.watermark-detail > summary')),true);
assert.equal(await page.evaluate(()=>window.__qa.windowControl.inner.width),2360,'Opening a miniature overlay must not enlarge the window');
console.log('PASS: mouse/keyboard folding preserves photo layout and watermark, remembers preference and avoids enlargement while collapsed.');

const withDock=(await geometry()).photo.width;
await page.click('.watermark-section .segmented button:first-child');await page.waitForSelector(dock,{state:'hidden'});
assert.equal(await geometryWithoutDock(),withDock,'Enabling or disabling the overlay never takes photo width');
async function geometryWithoutDock(){return page.evaluate(()=>document.querySelector('.photo-viewport').getBoundingClientRect().width);}
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
const boundary=await page.evaluate(()=>{const list=document.querySelector('.font-picker');list.scrollTop=list.scrollHeight;const r=list.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,panel:document.querySelector('#watermark-panel').scrollTop};});
await page.mouse.move(boundary.x,boundary.y,{label:'Scrolling within font options'});await page.mouse.wheel(0,380);
assert.equal(await page.evaluate(()=>document.querySelector('#watermark-panel').scrollTop),boundary.panel);
visibleTogether(await geometry());
await page.selectOption('.language-select','zh-CN');
assert.ok(await page.evaluate(()=>document.querySelector('.detail-heading').getAttribute('aria-label').includes('收起水印预览')));
console.log({screenshot:await page.screenshot({path:'/tmp/liubai-corner-watermark-verified.png'})});
console.log('All watermark workspace checks passed. Photo/font/render replies were isolated fixtures; no model calls.');
