// Real Svelte window routing with synthetic, explicitly gated native boundaries.
// Does not establish real OS-picker or native-file persistence behavior.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin MCP reliability');console.log({spaceId:task.spaceId});
const page=task.page('p1');await page.goto('http://127.0.0.1:1420/tests/ui/app.html?agent-conflict='+Date.now());
await page.waitForFunction(()=>window.__qa?.agentControl.ready&&document.querySelector('.zoom-photo')?.complete&&document.querySelector('.import-button')?.disabled===false);
console.log(await page.snapshot());await page.selectOption('.language-select','en');
await page.evaluate(()=>{window.__qa.agentState.enabled=true;});
const call=(name,args={})=>page.evaluate(({name,args})=>window.__qa.agent(name,args),{name,args});
await call('margin_import_photos',{paths:Array.from({length:26},(_,i)=>'/synthetic/conflict-'+i+'.png')});
for(let i=0;i<20;i++)await call('margin_remove_photos');
const baseline=await call('margin_get_workspace');assert.equal(baseline.history.undoCount,20);assert.equal(baseline.photoCount,7);
const unchanged=async()=>{
  const current=await call('margin_get_workspace');
  assert.deepEqual(current.photos,baseline.photos);assert.deepEqual(current.selection,baseline.selection);assert.deepEqual(current.history,baseline.history);
  assert.deepEqual(await page.evaluate(()=>window.__qa.catalog.history),history);
  return current;
};
const history=await page.evaluate(()=>window.__qa.catalog.history);

// A dialog opened AFTER the initial agent preflight must still protect the UI.
await page.evaluate(()=>{window.__qa.settingsControl.hold=true;});
await page.fill('input[aria-label="Border padding percentage"]','17');
await page.waitForFunction(()=>window.__qa.settingsControl.pending.length>0);
await page.evaluate(()=>{window.__qa.conflictResult=null;window.__qa.agent('margin_update_settings',{patch:{frameColor:'#E6DDC8'}}).then(value=>window.__qa.conflictResult={value},error=>window.__qa.conflictResult={error:String(error)});});
await page.waitForSelector('.agent-activity');
assert.equal(await page.evaluate(()=>document.querySelector('.workspace').inert),true);
await page.click('.model-config-trigger');await page.waitForSelector('.models-config[open]');
await page.evaluate(()=>{window.__qa.settingsControl.hold=false;for(const resolve of window.__qa.settingsControl.pending.splice(0))resolve();});
await page.waitForFunction(()=>!!window.__qa.conflictResult);
assert.match(await page.evaluate(()=>window.__qa.conflictResult.error??''),/dialog|对话框/);
let current=await unchanged();assert.equal(current.settings.borderPercent,17);assert.equal(current.settings.frameColor,baseline.settings.frameColor);
assert.equal(await page.evaluate(()=>window.__qa.savedSettings.borderPercent),17);
assert.equal(await page.evaluate(()=>document.querySelector('.models-config').open),true);
console.log('PASS: a dialog opened during a pending user save blocks the agent change and preserves all twenty undo records.');
await page.keyboard.press('Escape');await page.waitForSelector('.models-config[open]',{state:'hidden'});

// Without a conflict, flush the human edit first and merge the agent's patch.
await page.evaluate(()=>{window.__qa.settingsControl.hold=true;});
await page.fill('input[aria-label="Border padding percentage"]','18');
await page.waitForFunction(()=>window.__qa.settingsControl.pending.length>0);
await page.evaluate(()=>{window.__qa.conflictResult=null;window.__qa.agent('margin_update_settings',{patch:{frameColor:'#E6DDC8'}}).then(value=>window.__qa.conflictResult={value},error=>window.__qa.conflictResult={error:String(error)});});
await page.waitForSelector('.agent-activity');
await page.evaluate(()=>{window.__qa.settingsControl.hold=false;for(const resolve of window.__qa.settingsControl.pending.splice(0))resolve();});
await page.waitForFunction(()=>!!window.__qa.conflictResult);assert.equal(await page.evaluate(()=>!!window.__qa.conflictResult.error),false);
current=await unchanged();assert.equal(current.settings.borderPercent,18);assert.equal(current.settings.frameColor,'#E6DDC8');
assert.deepEqual(await page.evaluate(()=>({padding:window.__qa.savedSettings.borderPercent,color:window.__qa.savedSettings.frameColor})),{padding:18,color:'#E6DDC8'});
console.log('PASS: an outstanding human save is serialized before the agent patch without losing either field or history.');
console.log('Conflict-only checks finished; no production app or model account used.');
