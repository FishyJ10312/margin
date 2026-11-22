// Exercises the actual UI and Agent commands using an isolated native-transport fixture.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin settings consistency');
console.log({spaceId:task.spaceId});
const page=task.page('p1');
const call=(name,args={})=>page.evaluate(({name,args})=>window.__qa.agent(name,args),{name,args});
const photoSettings=()=>page.evaluate(()=>window.__qa.catalog.photos.map(p=>p.settings??null));
const select=(ids,active=ids.at(-1))=>call('margin_select_photos',{ids,activeId:active,anchorId:ids[0]});
const patch=patch=>call('margin_update_settings',{patch});
async function fresh(three=false){
 await page.goto('http://127.0.0.1:1420/tests/ui/app.html');
 await page.waitForFunction(()=>window.__qa?.agentControl.ready&&document.querySelector('.zoom-photo')?.complete);
 await page.selectOption('.language-select','en');
 await page.click('.agent-connect-trigger');await page.waitForSelector('#agent-enabled');await page.click('#agent-enabled');
 await page.waitForFunction(()=>window.__qa.agentState.enabled);
 await page.keyboard.press('Escape');await page.waitForSelector('.agent-connection[open]',{state:'hidden'});
 await call('margin_import_photos',{paths:three?['/synthetic/B.png','/synthetic/C.png']:['/synthetic/B.png']});
 const ids=(await call('margin_get_workspace')).photos.map(p=>p.id);
 await select(ids.slice(0,2),ids[0]);
 await patch({frameColor:'#FFFFFF',borderPercent:5,watermark:{kind:'text',text:'Group signature'}});
 await select([ids[1]]);
 await patch({frameColor:'#171A17',borderPercent:20,ratio:{width:1,height:1},watermark:{kind:'none'}});
 return ids;
}
const cases=[];
async function check(name,fn){
 try{await fn();cases.push({name,ok:true});console.log('PASS: '+name);}
 catch(error){cases.push({name,ok:false,error:String(error)});console.log('FAIL: '+name+' '+String(error));}
}
await check('Explicit Agent settings apply to every selected photo, even when the active one already matches',async()=>{
 const ids=await fresh();
 await select(ids,ids[0]);await patch({frameColor:'#FFFFFF',borderPercent:5,watermark:{kind:'text',text:'Group signature'}});
 const result=await photoSettings();
 assert.equal(result[0].frameColor,'#FFFFFF');assert.equal(result[1].frameColor,'#FFFFFF');
 assert.equal(result[1].borderPercent,5);assert.equal(result[1].watermark.kind,'text');assert.equal(result[1].watermark.text,'Group signature');
 assert.deepEqual(result[1].ratio,{width:1,height:1},'Unmentioned per-photo ratio is preserved');
});
await check('Applying a library style already present on the active photo updates the other selected photo',async()=>{
 const ids=await fresh();
 await select([ids[0]]);await call('margin_apply_template',{id:'builtin:keyline'});
 await select(ids,ids[0]);
 await page.click('.quick-templates-heading button');await page.waitForSelector('input[value="keyline"]');
 await page.focus('input[value="keyline"]');await page.press('input[value="keyline"]','Space');
 await page.waitForFunction(()=>document.querySelector('.preview-image')?.getAttribute('aria-busy')==='false');
 await page.click("loc=role:button[name='Apply selected style']");await page.waitForSelector('.templates-dialog',{state:'hidden'});
 await page.waitForFunction(()=>window.__qa.settingsControl.pending.length===0);
 await call('margin_get_workspace');
 // A following Agent action flushes the queued user edit.
 await select(ids,ids[0]);
 const result=await photoSettings();
 assert.equal(result[1].artwork?.design?.id,'keyline');
 assert.equal(result[1].frameColor,result[0].frameColor);
 assert.deepEqual(result[1].ratio,{width:1,height:1},'Keep layout means each photo keeps its own composition');
 assert.equal(result[1].borderPercent,20);
});
await check('Undo frame change restores each photo’s original independent border',async()=>{
 const ids=await fresh(),before=await photoSettings();
 await select(ids,ids[0]);await page.click('.quick-template-grid button >> nth=1');
 await page.waitForFunction(()=>!document.querySelector('.apply-trial')?.disabled);
 await page.click('.apply-trial');
 await page.waitForFunction(()=>window.__qa.catalog.photos.slice(0,2).every(p=>p.settings?.artwork?.design?.id==='keyline'));
 await page.click("loc=role:button[name='Undo frame change']");
 await page.waitForFunction(()=>window.__qa.catalog.photos[0].settings?.frameColor==='#FFFFFF');
 const after=await photoSettings();
 assert.deepEqual(after.slice(0,2).map(s=>({frameColor:s.frameColor,borderPercent:s.borderPercent,artwork:s.artwork,ratio:s.ratio})),before.slice(0,2).map(s=>({frameColor:s.frameColor,borderPercent:s.borderPercent,artwork:s.artwork,ratio:s.ratio})));
});
await check('Shared editing still previews the actual independently framed photo',async()=>{
 const ids=await fresh();await select([ids[1]]);
 await page.click('.settings-scope button >> nth=0');
 await call('margin_get_preview');
 const result=await page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd==='get_preview').at(-1).args.settings);
 assert.equal(result.frameColor,'#171A17');assert.equal(result.borderPercent,20);assert.deepEqual(result.ratio,{width:1,height:1});
});
await check('Catalog undo waits for an in-flight photo settings save and preserves the edit',async()=>{
 const ids=await fresh(true);
 await select([ids[2]]);await call('margin_remove_photos',{ids:[ids[2]]});await select([ids[1]]);
 await page.evaluate(()=>window.__qa.settingsControl.hold=true);
 await page.click('.palette-swatch[aria-label*="#F3ECDD"]');
 await page.waitForFunction(()=>window.__qa.settingsControl.pending.length===1);
 await page.click('.undo-photo-removal');
 await page.waitForFunction(()=>document.querySelector('.undo-photo-removal')?.disabled||window.__qa.requests.some(r=>r.cmd==='undo_photo_removal'));
 try{assert.equal(await page.evaluate(()=>window.__qa.requests.filter(r=>r.cmd==='undo_photo_removal').length),0,'Undo must not read the catalog before the pending settings commit');}
 finally{await page.evaluate(()=>{window.__qa.settingsControl.hold=false;window.__qa.settingsControl.pending.shift()();});}
 await page.waitForFunction(()=>document.querySelectorAll('.thumbnail').length===3);
 await select([ids[1]]);
 assert.equal(await page.evaluate(()=>document.querySelector('#frame-hex').value),'#F3ECDD');
});
await check('A failed Agent transaction never changes visible settings or becomes a deferred successful edit',async()=>{
 const ids=await fresh();await select(ids,ids[0]);
 const before=await photoSettings();
 await page.evaluate(()=>window.__qa.settingsControl.fail=true);
 const failure=await page.evaluate(async()=>{try{await window.__qa.agent('margin_update_settings',{patch:{frameColor:'#C99786',borderPercent:17}});return null;}catch(e){return String(e);}});
 assert.ok(failure);
 assert.equal((await call('margin_get_workspace')).settings.frameColor,'#FFFFFF');
 await page.evaluate(()=>window.__qa.settingsControl.fail=false);
 await select([ids[1]]);
 assert.deepEqual(await photoSettings(),before,'An unrelated successful operation must not persist an earlier failed Agent mutation');
});
assert.ok(cases.every(c=>c.ok),JSON.stringify(cases));
