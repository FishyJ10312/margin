// Real wheel input over the actual FontPicker inside the app's settings panel.
// Native fonts and photo handling are isolated fixture replies.
import assert from 'node:assert/strict';
const task=await taskSpace('Liubai font scroll containment');
console.log({spaceId:task.spaceId});
const page=task.page('p1');
async function prepare(query='?long-font-list'){
 await page.goto('http://127.0.0.1:1420/tests/ui/app.html'+query);
 await page.cdp('Emulation.setDeviceMetricsOverride',{width:1180,height:800,deviceScaleFactor:1,mobile:false});
 await page.waitForSelector('.quick-templates-heading button:not([disabled])');
 await page.selectOption('.language-select','en');
 await page.click('#inspector-watermark');
 await page.waitForSelector('.font-picker [data-index="0"]');
 await page.click('.position-details > summary');
 assert.ok(await page.evaluate(()=>{const panel=document.querySelector('#watermark-panel');return panel.scrollHeight>panel.clientHeight+100;}),'Expanded position controls provide a real outer scroll region');
 if(query.includes('legacy-scroll'))await page.evaluate(()=>document.querySelector('.font-picker').style.overscrollBehavior='auto');
}
async function aim(edge){
 const pos=await page.evaluate(edge=>{
  const list=document.querySelector('.font-picker'),panel=document.querySelector('#watermark-panel');
  list.scrollIntoView({block:'center',behavior:'instant'});list.scrollTop=edge==='bottom'?list.scrollHeight:0;
  const r=list.getBoundingClientRect(),p=panel.getBoundingClientRect();
  return {x:r.x+r.width/2,y:r.y+r.height/2,outsideX:p.x+5,panelTop:panel.scrollTop,listTop:list.scrollTop,listMax:list.scrollHeight-list.clientHeight};
 },edge);
 await page.mouse.move(pos.x,pos.y,{label:'Scrolling inside font options'});
 // Let a preceding outer-wheel gesture finish before taking a new baseline.
 // Otherwise a fractional compositor remainder can be mistaken for chaining.
 const stable=await page.evaluate(()=>new Promise(resolve=>{let last=-1,same=0,frames=0;const frame=()=>{const panel=document.querySelector('#watermark-panel'),list=document.querySelector('.font-picker');const now=panel.scrollTop;same=now===last?same+1:0;last=now;if(same>=6||++frames>=60)resolve({panelTop:now,listTop:list.scrollTop});else requestAnimationFrame(frame);};requestAnimationFrame(frame);}));
 return {...pos,...stable};
}
async function tops(){return page.evaluate(()=>({panel:document.querySelector('#watermark-panel').scrollTop,list:document.querySelector('.font-picker').scrollTop}));}
async function wheel(delta){await page.mouse.wheel(0,delta);await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));}
await prepare();
console.log((await page.snapshot({scope:'full_page'})).slice(-1400));
for(const [edge,delta] of [['bottom',380],['top',-380]]){
 const before=await aim(edge);assert.ok(before.listMax>1000);
 await wheel(delta);const after=await tops();console.log({edge,beforePanel:before.panelTop,afterPanel:after.panel,list:after.list});
 assert.equal(after.panel,before.panelTop,'At the '+edge+' of the font list, wheel must not move the settings page');
 await page.mouse.move(before.outsideX,before.y,{label:'Scrolling outside font options'});await wheel(delta);const outside=await tops();
 assert.notEqual(outside.panel,after.panel,'Moving the pointer outside the font list must allow page scrolling');
}
const middle=await aim('top');await wheel(180);const moving=await tops();assert.ok(moving.list>0);assert.equal(moving.panel,middle.panelTop);
await page.focus('.font-picker');await page.press('.font-picker','End');await page.waitForFunction(()=>document.querySelector('.font-picker [role=option][aria-selected=true]')?.textContent.includes('QA Font 80'));
console.log('PASS: long list stays contained at both boundaries, scrolls normally within, releases outside, and keeps keyboard selection.');
for(const query of ['', '?long-font-list&legacy-scroll']){
 await prepare(query);const before=await aim('bottom');await wheel(380);const after=await tops();assert.equal(after.panel,before.panelTop);
 if(!query){await page.fill('#font-search','No font with this name');const empty=await aim('bottom');await wheel(380);assert.equal((await tops()).panel,empty.panelTop);}
 console.log('PASS:',query?'compatibility fallback':'short and empty font lists');
}
